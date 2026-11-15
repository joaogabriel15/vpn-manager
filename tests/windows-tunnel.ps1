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
    Step 'Install RRAS'
    Install-WindowsFeature RemoteAccess, DirectAccess-VPN, Routing -IncludeManagementTools | Out-Null
    Install-RemoteAccess -VpnType Vpn -IPAddressRange '10.77.0.10', '10.77.0.50'
    net user $user $password /add /passwordchg:no | Out-Null
    netsh ras set user name=$user dialin=permit | Out-Null
    $server = (Get-NetIPAddress -AddressFamily IPv4 | Where-Object { $_.PrefixOrigin -ne 'WellKnown' -and $_.IPAddress -notlike '169.254.*' } | Select-Object -First 1).IPAddress
    Write-Host "Server address: $server"

    Step "Create $TunnelType client profile with stored credentials"
    Add-VpnConnection -Name $name -ServerAddress $server -TunnelType $TunnelType -AuthenticationMethod MSChapv2 -EncryptionLevel Required -SplitTunneling -RememberCredential -Force
    Install-PackageProvider NuGet -Force -Scope CurrentUser | Out-Null
    Install-Module VPNCredentialsHelper -Force -Scope CurrentUser
    Import-Module VPNCredentialsHelper
    Set-VpnConnectionUsernamePassword -connectionname $name -username $user -password $password -domain '' | Out-Null
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
