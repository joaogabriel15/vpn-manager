# Releases desktop

O workflow **Desktop releases** compila/testa Linux x86_64, Windows x86_64, macOS Intel e Apple Silicon em runners nativos. Execução manual gera artefatos para revisão, sem publicação. Cada push na `main` e cada tag `v<versão do Cargo.toml>` gera e publica uma **prerelease** somente depois que os quatro builds passam e todos os arquivos/checksums são verificados. Os pacotes levam um inventário de dependências e seus arquivos de licença disponíveis; a decisão de licença do código do app permanece pendente em REL-01.

## Instalação

- **Linux:** requer GTK >= 4.12, NetworkManager/nmcli e os plugins VPN do sistema. Extraia o `.tar.gz`, execute `./install.sh` na pasta extraída e abra pelo menu. O instalador é por usuário. Para remover, apague `~/.local/bin/vpn-manager`, `~/.local/bin/vpn-manager-cli`, a entrada `br.com.pacheco.VpnManager.desktop` em `~/.local/share/applications` e os ícones `vpn-manager` em `~/.local/share/icons/hicolor`. Perfis do NetworkManager permanecem intactos. O pacote inclui `LINUX-VPN-SUPPORT.md`, com os fluxos de cadastro/autenticação e limites por protocolo. O pacote é compilado no Ubuntu 24.04 (glibc 2.39); não é um binário estático universal.
- **Windows:** extraia todo o `.zip` em uma pasta do usuário e execute `vpn-manager.exe`. A CLI é `vpn-manager-cli.exe`. Requer Windows com Windows PowerShell 5.1, módulo VpnClient e RAS, conforme disponíveis no Windows 10/11; versões não foram homologadas. O pacote é portátil, sem serviço ou instalador e sem assinatura Authenticode. Configure sua VPN e as credenciais nas configurações de rede do Windows. Somente perfis VPN do usuário são listados; perfis globais não são gerenciados. Não há elevação automática. Remova a pasta extraída para desinstalar; as VPNs do sistema são preservadas.
- **macOS 13+:** use o `.zip` da sua arquitetura, extraia e mova `VPN Manager.app` para Applications. A CLI está em `VPN Manager.app/Contents/MacOS/vpn-manager-cli`. Somente perfis expostos por `scutil --nc` são gerenciados. Configure VPNs/certificados e credenciais no sistema. Assinatura ad hoc, sem Developer ID/notarização; Gatekeeper ou políticas corporativas podem impedir a execução. Remova o `.app` para desinstalar; as VPNs do sistema são preservadas.

Windows/macOS têm interface simples com busca, atualização manual e ações de conectar/desconectar. Operações rodam fora da thread gráfica. O app não pede nem armazena senhas nessas plataformas. Autenticação interativa/MFA, engines OpenVPN/WireGuard próprias, importação, bandeja e eventos de rede nessas plataformas continuam pendentes. Solicitar conexão não comprova túnel: clique **Atualizar** e confira o estado retornado pelo sistema. Fechar o gerenciador não desliga a VPN do sistema.

## Integridade

Linux/macOS: `shasum -a 256 <arquivo>` e compare com o `.sha256` da mesma release. No Windows: `Get-FileHash <arquivo> -Algorithm SHA256`. Checksums detectam corrupção; não substituem assinatura de código.

## Publicação automática na main

Cada push na `main` publica uma prerelease própria após os quatro builds e a verificação dos checksums passarem. A tag automática tem o formato `main-<execução>-<tentativa>-<commit-curto>` e aponta para o commit exato do push. Reexecuções recebem tags distintas, sem sobrescrever releases anteriores. Execuções de commits diferentes não se cancelam entre si.

A versão dos binários e dos arquivos continua sendo a declarada no Cargo.toml; a tag identifica cada build automático sem alterar versões no código. Execução manual continua gerando somente artefatos, sem publicação.

## Gerar uma release por versão

1. Atualize as versões dos quatro pacotes Cargo, `Cargo.lock` e `docs/RELEASE-NOTES.md`.
2. Envie as alterações e rode **Desktop releases** manualmente. Revise os quatro artefatos e teste as plataformas de destino.
3. Crie e envie a tag correspondente, por exemplo `git tag v0.2.0` e `git push origin v0.2.0` para a versão 0.2.0.
4. O workflow publica como prerelease. Falhas em qualquer build impedem a publicação. Tags já publicadas não são sobrescritas; use uma nova versão. Se o upload falhar, o rascunho pode permanecer para inspeção antes de uma nova tentativa.

Para compilar localmente Windows/macOS: `cargo build --release --locked -p vpn-manager -p vpn-manager-native`. Linux: `cargo build --release --locked -p vpn-manager -p vpn-manager-networkmanager`. O núcleo e os parsers podem ser testados sem GTK com `cargo test -p vpn-manager-core -p vpn-manager-native --locked`.

A existência de um pacote não homologa conectividade, DNS, IPv6, suspensão ou reconexão. A matriz de QA continua necessária em cada sistema.

Referências de integração: [Get-VpnConnection](https://learn.microsoft.com/en-us/powershell/module/vpnclient/get-vpnconnection), [rasdial](https://learn.microsoft.com/en-us/previous-versions/windows/it-pro/windows-server-2012-r2-and-2012/ff859533(v=ws.11)) e [implementação scutil da Apple](https://github.com/apple-oss-distributions/configd/blob/main/scutil.tproj/nc.c). A interface Windows/macOS usa [eframe/egui](https://github.com/emilk/egui).
