# End-to-end tunnel on a disposable Windows Server runner: the app CLI drives the
# native client (rasdial) against a local RRAS server. Never run on a workstation.
param([Parameter(Mandatory = $true)][string]$Cli, [string]$TunnelType = 'Pptp')
$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $false
$name = "VPN Manager lab $TunnelType"
$user = 'vpnlab'
$password = 'Lab-' + [guid]::NewGuid().ToString('N')
function Step($text) { Write-Host "== $text" }
function Wait-State($expected) {
    for ($i = 0; $i -lt 30; $i++) {
        $line = @(& $Cli list | Where-Object { $_.StartsWith($script:id) })
        if ($line.Count -eq 1 -and $line[0].Split("`t")[1] -eq $expected) { return $line[0] }
        Start-Sleep -Seconds 2
    }
    throw "CLI never reported '$expected' (last: $line)"
}

try {
    Step 'Client RAS viability before RRAS'
    Get-PnpDevice -Class Net -ErrorAction SilentlyContinue | Where-Object FriendlyName -like 'WAN Miniport*' | Format-Table -AutoSize Status, FriendlyName
    Set-Service RasMan -StartupType Manual
    sc.exe start RasMan | Select-String STATE
    Start-Sleep -Seconds 5
    sc.exe query RasMan | Select-String 'STATE|EXIT_CODE'

    Step 'Install RRAS'
    Install-WindowsFeature RemoteAccess, DirectAccess-VPN, Routing -IncludeManagementTools | Out-Null
    Install-RemoteAccess -VpnType Vpn -IPAddressRange '10.77.0.10', '10.77.0.50'
    net user $user $password /add /passwordchg:no | Out-Null
    netsh ras set user name=$user dialin=permit | Out-Null
    Get-RemoteAccess | Format-List VpnStatus, VpnS2SStatus
    Write-Host "RemoteAccess service: $((Get-Service RemoteAccess).Status)"
    $route = Get-NetRoute -DestinationPrefix '0.0.0.0/0' | Sort-Object RouteMetric | Select-Object -First 1
    $server = (Get-NetIPAddress -AddressFamily IPv4 -InterfaceIndex $route.ifIndex | Select-Object -First 1).IPAddress
    Write-Host "Server address: $server"

    Step 'Enable client services disabled in the runner image'
    function Enable-ServiceTree($svcName) {
        $s = Get-Service $svcName
        foreach ($dep in $s.ServicesDependedOn) { Enable-ServiceTree $dep.Name }
        if ($s.StartType -eq 'Disabled') { Set-Service $svcName -StartupType Manual }
        if ((Get-Service $svcName).Status -ne 'Running') { Start-Service $svcName }
        Write-Host "$svcName was $($s.StartType)/$($s.Status), now $((Get-Service $svcName).Status)"
    }
    try { Enable-ServiceTree 'RasMan' } catch {
        Write-Host '-- RasMan diagnostics'
        sc.exe qc RasMan
        sc.exe start RasMan
        reg query 'HKLM\SYSTEM\CurrentControlSet\Services\RasMan' /s | Select-Object -First 40
        Get-WinEvent -FilterHashtable @{ LogName = 'System'; ProviderName = 'Service Control Manager' } -MaxEvents 30 |
            Where-Object { $_.Message -match 'Remote Access|RasMan|Telephony' } | Format-List TimeCreated, Id, Message
        throw
    }

    Step "Create $TunnelType client profile with stored credentials"
    Add-VpnConnection -Name $name -ServerAddress $server -TunnelType $TunnelType -AuthenticationMethod MSChapv2 -EncryptionLevel Required -SplitTunneling -RememberCredential -Force
    # Same call Windows makes for "remember credentials"; rasdial reuses them when no user is given.
    Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class RasLab {
    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    public struct RASCREDENTIALS {
        public int dwSize; public int dwMask;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 257)] public string szUserName;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 257)] public string szPassword;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 16)] public string szDomain;
    }
    [DllImport("rasapi32.dll", CharSet = CharSet.Unicode)]
    static extern int RasSetCredentials(string phonebook, string entry, ref RASCREDENTIALS credentials, bool clear);
    public static int Save(string phonebook, string entry, string user, string password) {
        var c = new RASCREDENTIALS { dwMask = 7, szUserName = user, szPassword = password, szDomain = "" };
        c.dwSize = Marshal.SizeOf(c);
        return RasSetCredentials(phonebook, entry, ref c, false);
    }
}
'@
    $phonebook = Join-Path $env:APPDATA 'Microsoft\Network\Connections\Pbk\rasphone.pbk'
    $status = [RasLab]::Save($phonebook, $name, $user, $password)
    if ($status -ne 0) { throw "RasSetCredentials failed: $status" }
    $script:id = ([guid](Get-VpnConnection -Name $name).Guid).ToString()

    Step 'Connect through the app CLI'
    & $Cli connect $script:id
    if ($LASTEXITCODE -ne 0) {
        Write-Host 'CLI connect failed; raw rasdial diagnostic:'
        rasdial $name
        throw 'CLI connect failed'
    }
    Write-Host (Wait-State 'Conectada')
    $address = Get-NetIPAddress -AddressFamily IPv4 -InterfaceAlias $name
    Write-Host "Tunnel address: $($address.IPAddress)"
    if ($address.IPAddress -notlike '10.77.0.*') { throw 'Tunnel address is not from the lab pool' }
    if (-not (Test-Connection 10.77.0.10 -Count 2 -Quiet)) { throw 'No traffic through the tunnel' }

    Step 'Disconnect through the app CLI'
    & $Cli disconnect $script:id
    if ($LASTEXITCODE -ne 0) { throw 'CLI disconnect failed' }
    Write-Host (Wait-State 'Desconectada')
    if (Get-NetIPAddress -AddressFamily IPv4 -InterfaceAlias $name -ErrorAction SilentlyContinue) { throw 'Tunnel interface survived disconnect' }
    Write-Host 'WINDOWS_TUNNEL_OK'
} catch {
    Get-WinEvent -FilterHashtable @{ LogName = 'System'; ProviderName = 'RasMan', 'RemoteAccess', 'RasClient' } -MaxEvents 15 -ErrorAction SilentlyContinue | Format-List TimeCreated, Id, Message
    Get-WinEvent -FilterHashtable @{ LogName = 'Application'; ProviderName = 'RasClient' } -MaxEvents 10 -ErrorAction SilentlyContinue | Format-List TimeCreated, Id, Message
    throw
} finally {
    rasdial $name /disconnect 2>$null | Out-Null
    Remove-VpnConnection -Name $name -Force -ErrorAction SilentlyContinue
    $global:LASTEXITCODE = 0
}
