# Plataformas: plano de expansão

Pesquisa revisada em **12/11/2026**. Tudo abaixo, exceto a implementação Linux existente, é **planejamento**. Não há binários ou compatibilidade entregues para os demais sistemas.

## Meta e níveis de entrega

Prioridade atual, definida pelo proprietário: **Linux, Windows e macOS**. **Android e iOS/iPadOS estão adiados para uma fase futura**; requisitos móveis não bloqueiam a entrega desktop. Expansão posterior: BSD, ChromeOS e sistemas de roteadores/servidores. Outras plataformas passam por descoberta de APIs e demanda. Não prometer o mesmo protocolo, número de túneis simultâneos ou gerenciamento de outros clientes em todos os sistemas.

| Plataforma | Caminho técnico a avaliar | Entrega mínima proposta | Itens |
| --- | --- | --- | --- |
| Linux desktop | Manter GTK; abstrair NetworkManager e detectar recursos disponíveis. | Preservar o app atual, depois homologar OpenVPN, WireGuard, IKEv2 e conectores empresariais por versão. | OS-01, CORE-01/02 |
| Linux sem NetworkManager / headless | CLI e adaptadores de serviços, sem depender de sessão gráfica. | Estado, conexão, desligamento e configuração por canal autenticado; daemon opcional. | OS-06 |
| Windows | Serviço com privilégios mínimos; APIs VPN nativas e componentes oficiais para outros protocolos. | GUI/tray, OpenVPN e WireGuard; depois IKEv2 e conectores homologados. Arquiteturas x64 e ARM64 avaliadas separadamente. | OS-02 |
| macOS | NetworkExtension e componentes auxiliares permitidos, com assinatura e distribuição definidas. | App de menu, OpenVPN/WireGuard/IKEv2 conforme viabilidade e homologação. Apple Silicon e Intel conforme suporte vigente escolhido. | OS-03 |
| Android (adiado) | VpnService ou APIs de VPN da plataforma; camada de integração com o núcleo Rust. | Consentimento, início/parada pelo sistema, estado, reconexão, importação e gestão de credenciais. | OS-04 |
| iOS/iPadOS (adiado) | NetworkExtension/NEVPNManager e extensões de túnel permitidas. | App móvel e extensão assinada; protocolos somente depois de integração e testes reais. | OS-05 |
| ChromeOS (posterior) | Avaliar app Android suportado pelo dispositivo ou integração de configuração permitida. | Matriz por modelo/canal; não assumir que executar Linux no contêiner controla a VPN do host. | OS-07 |
| FreeBSD | Adaptadores locais e CLI; GUI só após validar dependências. | WireGuard/OpenVPN/IPsec conforme engines disponíveis e testes no sistema. | OS-08 |
| OpenBSD / NetBSD | Pesquisa independente de APIs e engines. | Registrar quais operações podem ser implementadas; não herdar aprovação de FreeBSD. | OS-08 |
| OpenWrt, roteadores Linux e NAS | CLI/serviço e integração de gerenciamento, conforme arquitetura e permissões do dispositivo. | Pacotes compatíveis e controle local/remoto autenticado; sem exigir GUI desktop. | OS-09 |
| Outros sistemas de roteador | Conectores por API oferecida pelo fabricante ou exportação de configuração compatível. | Não presumir instalação de um binário Rust ou acesso ao kernel em qualquer equipamento. | OS-09 |
| Navegador / web | Painel de um agente local/remoto autorizado. | Exibir claramente qual dispositivo está sendo controlado; navegador sozinho não representa uma VPN de todo o sistema. | OS-10 |
| TVs, wearables, consoles e plataformas adicionais | Descoberta de API, distribuição e casos de uso antes de escolher uma solução. | Classificar como app, controlador remoto, uso de gateway ou indisponível; não anunciar suporte só por compartilhar um kernel. | OS-11 |

As linhas “caminho técnico” e “entrega mínima” são decisões propostas, não capacidades já comprovadas do VPN Manager.

## Restrições que mudam a arquitetura

### Windows

O Windows oferece uma plataforma VPN nativa e extensões de terceiros. Wintun fornece uma interface de pacotes para engines próprias; não implementa autenticação ou um protocolo VPN completo. Para WireGuard, o projeto recomenda seu serviço incorporável em vez de integrar WireGuardNT diretamente. Revisar requisitos de assinatura, instalação e licença antes de distribuir componentes. [Microsoft](https://learn.microsoft.com/en-us/windows/security/operating-system-security/network-security/vpn/vpn-connection-type), [Wintun](https://www.wintun.net/), [WireGuard](https://www.wireguard.com/embedding/).

### Apple

A integração proposta usa NetworkExtension e os recursos permitidos para o tipo de distribuição. A diretriz 5.4 da App Store exige organização para apps que oferecem serviços VPN. Confirmar enquadramento, conta, entitlements e processo de distribuição no início da fase, antes de prometer publicação. A conta pessoal no GitHub não resolve esse requisito. [NetworkExtension](https://developer.apple.com/documentation/xcode/configuring-network-extensions), [App Review, 5.4](https://developer.apple.com/app-store/review/guidelines/#vpn-apps).

Não presumir que o app possa listar, editar ou comandar os perfis de todos os outros apps instalados. Qualquer integração delegada precisa de API autorizada do fabricante e do sistema. O frontend pode ser nativo, enquanto regras, modelos e parte das engines compartilham Rust; não exigir uma GUI GTK em um aparelho móvel.

### Android — referência para fase futura

O VpnService requer consentimento do usuário, oferece integração com o sistema e limita a VPN ativa por usuário/perfil. Planejar corretamente serviço em foreground, notificação e ciclo de vida. O protocolo de túnel continua sendo responsabilidade da engine; há também APIs específicas de IKEv2. A distribuição pelo Google Play exige revisar a política de VpnService. [Guia Android](https://developer.android.com/develop/connectivity/vpn), [Ikev2VpnProfile](https://developer.android.com/reference/android/net/Ikev2VpnProfile), [política Google Play](https://support.google.com/googleplay/android-developer/answer/12564964).

Não transportar o modelo desktop de “executar o CLI do fabricante” diretamente para o celular. Preferir engines integráveis em nosso serviço; encaminhamento ao app oficial é uma capacidade distinta.

### ChromeOS e sistemas alternativos

Os recursos de VPN do ChromeOS variam; a documentação do Google descreve cliente integrado e alternativas Android. Testar o host, não somente a rede do contêiner de desenvolvimento. [ChromeOS](https://support.google.com/chromebook/answer/1282338?hl=en).

Para BSD/embedded, a existência de uma implementação de engine não prova o funcionamento da GUI, do cofre, de DNS, do firewall ou do empacotamento. Os componentes WireGuard e as matrizes de plataformas de outros projetos servem como referências para os estudos, não como certificados do app. [WireGuard](https://www.wireguard.com/embedding/), [ZeroTier](https://docs.zerotier.com/platforms/).

## Núcleo compartilhado e adaptadores

Estrutura-alvo, a validar por prova de conceito. A bridge móvel está fora da fase atual:

```text
modelos, perfis, validação e máquina de estados em Rust
                    |
       contrato de backend e eventos
          /          |
 NetworkManager  APIs/serviços  bridge móvel (fase futura)
 Linux/CLI        Windows/macOS  Android e iOS
                    |
          engines ou clientes autorizados
```

Separar operação do túnel, autenticação, controle de rotas/DNS e apresentação. A interface consulta capacidades reais: não exibir exportação, kill switch, múltiplos túneis ou controle de outro cliente se o adaptador não os oferece.

Escolher a estratégia de UI por comparação de protótipos em OS-00: manter o GTK Linux funcionando, medir memória e acessibilidade, avaliar UI compartilhada versus frontends nativos e registrar a decisão. Não reescrever a interface inteira antes de provar que as engines e APIs funcionam.

## Qualificação por sistema

Toda plataforma candidata precisa de uma versão mínima documentada, arquiteturas, canal de distribuição, dependências e um responsável pelo teste. Versões fora do ciclo de suporte do fabricante não entram automaticamente.

Para ganhar status homologado, testar instalação limpa, autorização negada/concedida, login, tráfego real, DNS e IPv6, suspensão, troca de rede, queda do daemon, reconexão, cancelamento, atualização e desinstalação. Protocolos e autenticações são homologados separadamente.

Medir GUI oculta/visível, serviço e engine de forma separada: RAM, CPU em repouso e conectado, consumo no celular e despertar de processos. Definir metas depois da medição dos protótipos; a medição Linux atual não é uma promessa de consumo para outros sistemas.
