param([Parameter(Mandatory = $true)][string]$Cli)
$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $false
$name = 'VPN Manager test ' + [guid]::NewGuid().ToString()
try {
    Add-VpnConnection -Name $name -ServerAddress 'vpn.example.invalid' -TunnelType Ikev2 -AuthenticationMethod Eap -RememberCredential:$false -Force | Out-Null
    $profile = Get-VpnConnection -Name $name
    $lines = & $Cli list
    if ($LASTEXITCODE -ne 0) { throw 'CLI inventory failed' }
    $found = @($lines | Where-Object { $_.Contains($profile.Guid.ToString()) -and $_.Contains($name) })
    if ($found.Count -ne 1) { throw 'Synthetic VPN was not found by stable ID and name' }
    & $Cli connect '00000000-0000-0000-0000-000000000000' 2>$null
    if ($LASTEXITCODE -eq 0) { throw 'Unknown profile was accepted' }
    # No connection, credential lookup or tunnel activation is performed.
} finally {
    if (Get-VpnConnection -Name $name -ErrorAction SilentlyContinue) {
        Remove-VpnConnection -Name $name -Force
    }
}
