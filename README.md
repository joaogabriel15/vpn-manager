# VPN Manager

Aplicativo Linux independente, escrito em Rust com GTK 4 e bandeja StatusNotifierItem. Gerencia perfis do NetworkManager sem executar como root. Interface em português.

## Interface

Janela compacta com busca, lista de VPNs e botão Conectar. O menu Adicionar reúne a importação e a criação de perfis; o menu do aplicativo reúne diagnóstico, início automático e saída. As configurações são organizadas nas abas Geral, Rede e DNS, e Avançado. Ícones SVG próprios, incorporados ao binário, mantêm a aparência consistente no app e na bandeja, independentemente do tema de ícones do Linux.

## Recursos

- Descobre os perfis VPN e WireGuard existentes no NetworkManager.
- Busca, conexão, desconexão, duplicação e remoção com confirmação.
- Importa OpenVPN `.ovpn` e WireGuard `.conf` usando os plugins do NetworkManager.
- Cria OpenVPN com usuário/senha, CA, porta e UDP/TCP.
- Edita nome, usuário, opções do plugin, DNS IPv4/IPv6, domínios de busca, rotas e túnel dividido.
- Abre o editor do Linux para certificados, peers WireGuard e demais opções específicas do protocolo (KDE ou nm-connection-editor).
- Exporta perfis dos plugins VPN que oferecem exportação; o arquivo tem permissão 0600. WireGuard não possui exportação nesta versão.
- Diagnóstico com interfaces, endereços, rotas e DNS.
- Ícone na bandeja com atalhos por conexão. Fechar a janela mantém o app em segundo plano; Sair encerra somente o gerenciador.
- Inicialização opcional na bandeja ao entrar na sessão.

## Roadmap

A versão atual é para Linux. A expansão para Windows, macOS, Android, iOS/iPadOS e outras famílias de VPN está planejada no [backlog](BACKLOG.md), com [inventário de compatibilidade](docs/VPN-COMPATIBILITY.md) e [plano por sistema operacional](docs/PLATFORMS.md). FortiClient/Fortinet tem tarefas separadas para SSL-VPN PPP, IPsec/SAML e políticas de endpoint. Esses documentos descrevem trabalho futuro, não suporte já disponível.

## Instalação

Requisitos: Rust, GTK >= 4.12, NetworkManager/nmcli e o plugin de cada protocolo. KDE Plasma já oferece suporte à bandeja; GNOME pode exigir uma extensão AppIndicator. No CachyOS, os pacotes relevantes são `gtk4`, `networkmanager` e `networkmanager-openvpn`. O editor avançado usa `plasma-nm` ou `nm-connection-editor`.

```sh
cargo build --release --locked
./install.sh
~/.local/bin/vpn-manager
```

O instalador é por usuário, sem sudo, e cria entradas no menu e na área de trabalho. O arquivo `br.com.pacheco.VpnManager.desktop` corresponde ao identificador da janela no Wayland. No KDE, o instalador também atualiza o atalho antigo já fixado na barra, preservando os outros atalhos e sua ordem. Marque **Iniciar com o Linux** no menu do app para ativar o autostart. `vpn-manager --background` abre diretamente na bandeja. Se a bandeja não estiver disponível, a janela aparece.

Para atualizar, recompile e execute o instalador novamente. O script reutiliza o binário já compilado. Os perfis são mantidos pelo NetworkManager e não são apagados ao remover o app.

## Uso

Clique em **Conectar** ou no nome da conexão no menu da bandeja. Para perfis VPN, o app pede o usuário e a senha. Deixe a senha vazia quando o perfil usa certificados ou credenciais já disponíveis no cofre do sistema. Para autenticação multifator ou desafios específicos, use o agente de segredos do ambiente Linux; o app não implementa um agente próprio.

Alterações de DNS e rotas entram em vigor ao reconectar. Túnel dividido impede a rota padrão pela VPN; configure as redes corporativas necessárias. O app não cria kill switch nem firewall e não promete bloquear tráfego quando a VPN cai. A inicialização automática inicia o gerenciador, não estabelece uma VPN automaticamente.

Na importação e exportação, certificados e chaves podem permanecer em arquivos externos referenciados pelo perfil. Preserve os arquivos e não compartilhe exportações sem revisar o conteúdo. O formato exportado depende do plugin instalado.

## Arquitetura e consumo

GTK desenha sob demanda. Um único `nmcli monitor` aguarda eventos de rede; não há consultas periódicas ou renderização contínua. Comandos são assíncronos pelo GIO, sem bloquear a interface. O NetworkManager mantém o túnel mesmo quando o gerenciador é encerrado.

Senhas não são salvas pelo app nem enviadas como argumentos de processo. Quando informadas, são entregues ao nmcli por um arquivo temporário 0600, removido após a tentativa. Perfis e certificados continuam sob controle do NetworkManager. Autorizações seguem as regras Polkit do sistema; o app não contorna permissões administrativas.

## Verificação

```sh
cargo test --locked
# Integração: cria e remove perfis temporários, sem conectar.
cargo test --locked -- --include-ignored
cargo clippy --all-targets --locked -- -D warnings
cargo build --release --locked
# Requer sessão gráfica e NetworkManager; consulta perfis e cancela o editor.
./target/release/vpn-manager --self-test
```

Para gerar capturas somente do app durante o teste, use `VPN_MANAGER_PREVIEW=/tmp/vpn-manager.png ./target/release/vpn-manager --self-test`. O teste também verifica o menu de ações, a busca, os ícones da bandeja e as abas do editor.

O teste gráfico executa uma instância separada, sem encerrar o app já aberto. Não conecta, remove ou altera perfis. A conexão real exige credenciais e acesso ao servidor.

## Remover

Desmarque o início automático no app, saia e remova `~/.local/bin/vpn-manager`, os arquivos `br.com.pacheco.VpnManager.desktop` e `vpn-manager.desktop` do menu/área de trabalho e o ícone instalado em `~/.local/share/icons/hicolor/scalable/apps/`. Os perfis VPN são preservados.

Referências técnicas: [GTK para Rust](https://gtk-rs.org/gtk4-rs/stable/latest/docs/gtk4/), [ksni](https://docs.rs/ksni/latest/ksni/), [nmcli](https://networkmanager.dev/docs/api/latest/nmcli.html).
