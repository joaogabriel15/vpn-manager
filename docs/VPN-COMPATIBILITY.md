# Inventário de VPNs e caminhos de integração

Pesquisa revisada em **12/11/2026**. Este documento descreve candidatos ao roadmap, não suporte já entregue. Prioridades e critérios estão em [BACKLOG.md](../BACKLOG.md); plataformas em [PLATFORMS.md](PLATFORMS.md).

## Como interpretar “todos os tipos”

Não existe um registro único de todos os produtos VPN, suas versões, extensões e políticas de autenticação. A meta é manter um catálogo extensível das famílias de protocolos e dos produtos relevantes, com evidência por combinação de sistema, backend, autenticação e gateway. Um protocolo compatível não garante acesso a qualquer equipamento que usa seu nome.

Há quatro integrações diferentes:

| Categoria | O que precisa funcionar |
| --- | --- |
| Protocolo | Negociar e manter um túnel com uma implementação compatível. |
| Produto/fabricante | Autenticação, provisionamento, portal, políticas e variantes daquele produto. |
| Mesh/overlay | Registro de dispositivos, identidade, coordenação, descoberta de peers e políticas. |
| Provedor comercial | Configuração autorizada, credenciais próprias do serviço e, quando disponível, API ou cliente oficial. |

“SSL VPN”, “IPsec”, “Zero Trust”, “site-to-site”, “VPN de acesso remoto” e “VPN por aplicativo” não são protocolos intercambiáveis. Os três últimos também descrevem cenários de uso. Criptografia de transporte, por si só, não descreve todo o mecanismo de acesso.

## Situação do app hoje

O app foi implementado para **Linux com GTK 4 e NetworkManager**. Há fluxos de OpenVPN e WireGuard; outros perfis `vpn` podem aparecer na lista, mas isso não comprova compatibilidade. O envio de senha ainda usa o campo específico `vpn.secrets.password`. Importação trata `.ovpn` como OpenVPN e pede confirmação para interpretar outros arquivos como WireGuard. Isso deve ser substituído por detecção explícita antes de ampliar o catálogo.

Testes locais cobrem interpretação da saída do nmcli, ciclo de vida de um perfil temporário e parte da interface. **Não há homologação de túneis reais, MFA, gateways empresariais ou versões para outros sistemas.**

## 1. Protocolos e túneis

Os caminhos abaixo são propostas de engenharia. A existência de um backend upstream não significa que ele já esteja integrado, que seja distribuível com o app ou que ofereça as mesmas funções em todos os sistemas.

| Família | Caminho proposto | Particularidades a testar | Backlog |
| --- | --- | --- | --- |
| OpenVPN | Plugin do NetworkManager no Linux; avaliar engines e serviços adequados aos demais sistemas. | UDP/TCP, certificados, senha, TLS-auth/TLS-crypt, chave privada protegida, MFA e opções de importação. | VPN-01 |
| WireGuard | NetworkManager/kernel no Linux; componentes de integração oficiais por plataforma. | Peers, AllowedIPs, PSK, chaves, keepalive, DNS, importação/exportação e QR code. | VPN-02 |
| IKEv2/IPsec | strongSwan no Linux; APIs nativas onde disponíveis. | EAP, certificado, PSK conforme backend, identidades, NAT-T, roaming e propostas criptográficas. | VPN-03 |
| L2TP/IPsec | Plugin/cliente compatível no Linux e APIs permitidas nos outros sistemas. | PSK/certificados, PPP, MTU, NAT e disponibilidade por versão do sistema. | VPN-04 |
| IKEv1/IPsec e Cisco IPsec legado | Adaptador específico, avaliando Libreswan e alternativas. | Não tratar IKEv1 como IKEv2 nem migrar perfis sem comparar opções. | LEG-01 |
| SSTP | Cliente nativo no Windows ou backend dedicado onde disponível. | PPP sobre TLS, certificados, proxy e manutenção da implementação. | LEG-01 |
| PPTP | Somente compatibilidade legada explicitamente habilitada, se viável. | Não oferecer como padrão nem reduzir a segurança de outro protocolo para fazê-lo conectar. | LEG-01 |
| SoftEther nativo | Avaliar cliente/API próprios; usar OpenVPN ou outro modo somente se o servidor o disponibilizar. | O protocolo SoftEther é diferente dos modos de compatibilidade do servidor. | ADV-01 |
| OpenSSH TUN | Adaptador para túnel de interface, distinto de encaminhamento SOCKS. | Permissões, autenticação por chave/agente e configuração do servidor. | ADV-02 |
| Iodine | Adaptador dedicado e laboratório separado. | Encapsulamento por DNS e proteção criptográfica adicional quando necessária. | ADV-02 |
| AmneziaWG | Engine e formato próprios, com versão explícita. | Não interpretar opções específicas como WireGuard padrão. | ADV-03 |
| Lightway | Avaliar implementação upstream e condições de integração. | API, licença, transporte e autenticação do serviço de destino. | ADV-03 |
| MASQUE / CONNECT-IP | Pesquisa de transporte e implementação compatível. | Um padrão de transporte não substitui registro e autenticação de um serviço como WARP. | ADV-03 |

Referências para os caminhos principais: [NetworkManager](https://networkmanager.dev/docs/vpn/), [integração WireGuard](https://www.wireguard.com/embedding/), [strongSwan/NetworkManager](https://docs.strongswan.org/docs/latest/features/networkManager.html), [VPN nativa Windows](https://learn.microsoft.com/en-us/windows/security/operating-system-security/network-security/vpn/vpn-connection-type), [SoftEther](https://www.softether.org/spec), [Amnezia](https://docs.amnezia.org/documentation/instructions/install-configure-protocols/), [Lightway](https://www.expressvpn.com/lightway) e [CONNECT-IP, RFC 9484](https://www.rfc-editor.org/rfc/rfc9484.html).

O catálogo atual do NetworkManager identifica plugins com problemas de manutenção e alerta sobre PPTP. A seleção de dependências precisa ser reavaliada antes de cada entrega; não assumir que todos os plugins listados estejam mantidos. [Fonte](https://networkmanager.dev/docs/vpn/).

## 2. VPNs empresariais e fabricantes

O OpenConnect documenta sete famílias: AnyConnect, Juniper Network Connect, Pulse/Ivanti, GlobalProtect, F5, Fortinet e Array. As limitações variam por família e versão; suporte do cliente de linha de comando também não garante que o plugin do NetworkManager exponha todas as mesmas opções. [Protocolos](https://www.infradead.org/openconnect/protocols.html), [manual](https://www.infradead.org/openconnect/manual.html).

| Produto/família | Integração candidata | Limite que precisa de evidência | Backlog |
| --- | --- | --- | --- |
| Cisco AnyConnect / Secure Client e servidores ocserv compatíveis | OpenConnect `anyconnect`; cliente oficial quando necessário. | Grupos, certificados, MFA e exigências específicas do gateway. | CORP-01 |
| Palo Alto GlobalProtect | OpenConnect `gp` ou integração oficial. | Portal e gateway são etapas diferentes; seleção de gateway, SAML e políticas do endpoint. | CORP-02 |
| Ivanti / Pulse Connect Secure | OpenConnect `pulse`. | Formulários, MFA, cookies e checagens exigidas pelo servidor. | CORP-03 |
| Juniper Network Connect | OpenConnect `nc`. | Compatibilidade legada e diferenças em relação ao protocolo Pulse. | CORP-03 |
| F5 BIG-IP APM | OpenConnect `f5` ou cliente oficial. | Domínio, certificado e políticas específicas. | CORP-03 |
| Array Networks AG | OpenConnect `array`. | Fluxo de autenticação e gateway homologado. | CORP-03 |
| Fortinet/FortiClient: SSL-VPN PPP | OpenConnect `fortinet`, depois de verificar a variante do servidor. | Não representa todo SSL-VPN Fortinet nem IPsec. | FORT-01 |
| Fortinet/FortiClient: IPsec | Adaptador IKE/IPsec e/ou cliente oficial. | IKE, SAML, versão, proposta criptográfica e autenticação suportados. | FORT-02 |
| Fortinet/FortiClient: EMS/ZTNA e políticas de endpoint | Pesquisa de integração oficial. | Não simular conformidade do endpoint nem declarar que uma conexão simples reproduz o FortiClient completo. | FORT-03 |
| Check Point | Investigar cliente oficial, SNX e modos IPsec em tarefas separadas. | Produtos, políticas e sistemas têm capacidades diferentes. | CORP-04 |
| SonicWall | Investigar NetExtender e modos de acesso do produto. | Confirmar versão e interface de automação oferecida pelo fabricante. | CORP-04 |
| Sophos Connect | Investigar perfis SSL/IPsec, provisionamento e SSO de forma independente. | Compatibilidade varia com versão de firewall, cliente, modo e autenticação. | CORP-05 |
| AWS Client VPN | Avaliar formatos compatíveis e uso do cliente oficial quando exigido. | OpenVPN genérico não atende automaticamente endpoints SAML. | CLOUD-01 |
| Azure VPN Gateway P2S | Selecionar protocolo e cliente conforme autenticação configurada. | Não confundir suporte ao transporte com suporte a login Entra ID. | CLOUD-01 |
| Outros fabricantes | Registro de demanda e levantamento por modelo/versão antes da implementação. | Não deduzir suporte por aparecer “SSL” ou “IPsec” no material comercial. | CORP-05 |

GlobalProtect: [documentação do OpenConnect](https://www.infradead.org/openconnect/globalprotect.html). Check Point: [SNX](https://sc1.checkpoint.com/documents/SSL_Network_Extender_AdminGuide/Content/Topics-SNX-Admin-Guide/Introduction-to-SNX.htm). SonicWall: [clientes oficiais](https://www.sonicwall.com/products/remote-access/vpn-clients).

Sophos: [release notes do fabricante](https://docs.sophos.com/releasenotes/output/en-us/nsg/connect_rn.html). AWS documenta a limitação de clientes OpenVPN genéricos em endpoints com SAML; Azure descreve as combinações de protocolo e autenticação para P2S. São casos explícitos de homologação de produto além do protocolo. [AWS Client VPN](https://docs.aws.amazon.com/vpn/latest/clientvpn-user/connect.html), [Azure P2S](https://learn.microsoft.com/en-us/azure/vpn-gateway/point-to-site-about).

### FortiClient: resposta específica

**FortiClient ainda não está homologado no app.** A marca do cliente não basta para escolher um backend. O levantamento precisa registrar modo SSL/IPsec, versão do gateway, método de login e eventual dependência do cliente oficial. O OpenConnect descreve o suporte Fortinet antigo baseado em PPP e suas limitações. A Fortinet documenta separadamente autenticação SAML para acesso IPsec, com dependências de versão. [OpenConnect/Fortinet](https://www.infradead.org/openconnect/fortinet.html), [Fortinet/IPsec e SAML](https://docs.fortinet.com/document/fortigate/latest/administration-guide/951346).

## 3. Mesh, overlays e acesso por identidade

Esses produtos precisam de conectores próprios. Importar uma configuração WireGuard não implementa o controle de identidade e da rede mesh.

| Produto/família | Caminho proposto | O que não pode ser confundido | Backlog |
| --- | --- | --- | --- |
| Tailscale; controle Headscale quando compatível | API/serviço local e registro autorizado no controle escolhido. | O plano de controle e as políticas não são uma configuração WireGuard simples. | MESH-01 |
| NetBird | Integração com serviço/cliente e registro na instância. | Identidade, sinalização e relays também participam da conexão. | MESH-02 |
| ZeroTier | API/cliente local, adesão à rede e autorização. | Tem modelo de rede próprio; não é um peer WireGuard. | MESH-03 |
| Nebula | Engine e gerenciamento de identidade/configuração. | Certificados, grupos e descoberta fazem parte do produto. | MESH-04 |
| tinc | Adaptador do daemon e configuração de peers. | Não equivale a OpenVPN. | MESH-04 |
| n2n | Pesquisa do cliente e da comunidade/rede. | Requer engine própria e confirmação da manutenção atual. | MESH-04 |
| Outras soluções ZTNA/overlay | Descoberta por demanda; SDK/API oficial antes de assumir controle do túnel. | Alguns produtos dão acesso a aplicações, não uma rede IP completa. | MESH-05 |

Fontes: [Tailscale e WireGuard](https://tailscale.com/docs/concepts/wireguard), [arquitetura NetBird](https://docs.netbird.io/about-netbird/how-netbird-works), [plataformas ZeroTier](https://docs.zerotier.com/platforms/), [Nebula](https://nebula.defined.net/docs/), [tinc](https://tinc-vpn.org/documentation/Introduction.html) e [n2n](https://github.com/ntop/n2n).

## 4. Provedores e variantes de protocolo

| Categoria | Entrega proposta | Backlog |
| --- | --- | --- |
| Provedor que exporta OpenVPN/WireGuard | Importar o formato autorizado; orientar quando as credenciais de VPN diferem do login do site. Validar por provedor e plano. | PROV-01 |
| NordLynx / NordWhisper | Investigar cliente/API e requisitos do serviço; não inferir suporte apenas pela existência de WireGuard. | PROV-02 |
| Proton Stealth / variantes de transporte | Registrar formatos, engines, autorização e sistemas por versão antes de integrar. | PROV-02 |
| Cloudflare WARP / Cloudflare One | Preferir interfaces oficiais do cliente; tratar registro e modo do serviço. | PROV-03 |
| Outros serviços e protocolos exclusivos | Conector somente com interface disponível e condições de distribuição conhecidas. Caso contrário, indicar abertura do cliente oficial. | PROV-02 |

O fornecedor pode mudar seus protocolos sem preservar um formato público de importação. A página da NordVPN descreve NordLynx e NordWhisper; a do WARP distingue seus modos e transportes. Essas características não tornam os serviços automaticamente compatíveis com o app. [NordVPN](https://support.nordvpn.com/hc/en-us/articles/19482810153745-Which-NordVPN-protocol-should-I-choose), [WARP/Linux](https://developers.cloudflare.com/warp-client/get-started/linux/), [protocolos Proton](https://protonvpn.com/support/protonvpn-features/protocols).

## 5. Proxies, transportes e infraestrutura adjacente

Entradas para pesquisa, sem promessa de VPN completa: SOCKS5, HTTP CONNECT, encaminhamento SSH, Shadowsocks, VMess, VLESS, Trojan, Hysteria e TUIC. Uma integração teria de declarar o escopo do tráfego e avaliar engine, TUN, DNS e UDP. Configurar apenas um proxy de aplicativo não significa que todo o sistema passa por um túnel. O suporte TUN de uma engine é um componente dessa solução, não a homologação do produto. [Exemplo de engine com TUN](https://sing-box.sagernet.org/configuration/inbound/tun/). Backlog: PROXY-01.

MPLS L2VPN/L3VPN, EVPN, VXLAN, GRE, L2TPv3 e EtherIP entram como levantamento de infraestrutura/gateways, não como promessa de um cliente desktop universal. Também não presumir criptografia pelo nome “túnel”. Pesquisa futura precisa decidir entre interoperar com um gateway existente e gerenciar infraestrutura remota. Backlog: EDGE-02.

## Registro obrigatório de compatibilidade

Cada resultado deve conter: sistema e versão, arquitetura, engine/plugin e versão, gateway e versão, autenticação, transporte, escopo de rotas/DNS/IPv6, modo de integração, testes executados, limitações e data. Não guardar senhas, cookies, chaves privadas, endereços corporativos ou nomes de clientes no repositório público.

Estados publicados: **planejado**, **em pesquisa**, **experimental**, **homologado com limitações**, **homologado**, **indisponível por restrição da plataforma**. Abertura de um app externo deve ser identificada como **integração delegada**, nunca como túnel implementado pelo VPN Manager.

Este inventário deve ser revisto a cada release que altera um backend e, enquanto houver desenvolvimento ativo, pelo menos trimestralmente. Novos fabricantes entram pelo mesmo processo, sem limitar o catálogo aos nomes desta pesquisa.
