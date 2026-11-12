# Backlog — VPN Manager multiplataforma e multiprotocolo

**Revisão:** 12/11/2026. **Estado:** planejamento; os itens pendentes não foram implementados por este levantamento.

Objetivo: permitir gerenciar o maior conjunto viável de VPNs com uma interface simples e núcleo compartilhado em Rust, preservando baixo consumo e respeitando as capacidades de cada sistema. A meta principal cobre Linux, Windows, macOS, Android e iOS/iPadOS; plataformas adicionais entram por adaptadores e estudos específicos.

- [Inventário de protocolos, fabricantes, mesh e provedores](docs/VPN-COMPATIBILITY.md)
- [Sistemas operacionais, arquitetura e restrições](docs/PLATFORMS.md)

“Todos” significa um processo extensível de integração e homologação, não uma promessa de qualquer produto em qualquer versão de sistema. Bloqueios por API, licença, política de distribuição ou exigência de cliente oficial devem ficar explícitos na matriz de suporte.

## Base existente

- [x] App Linux em Rust/GTK, com NetworkManager, busca e bandeja.
- [x] Fluxos de criação/importação OpenVPN e importação WireGuard.
- [x] Edição de propriedades de rede, duplicação, exportação compatível e remoção de perfis.
- [x] Testes locais de parsing, perfis temporários e interface, incluindo rodapé de modal em janela reduzida.
- [x] Pesquisa inicial e separação das famílias e plataformas neste backlog.

Essas entregas não equivalem a homologação de servidores reais. Hoje não há comprovação de suporte a FortiClient, MFA, versões desktop fora do Linux ou mobile. A tarefa de homologação começa em QA-01/02.

## Prioridade, esforço e acompanhamento

| Campo | Convenção |
| --- | --- |
| P0 | Base e decisões necessárias para ampliar o app sem acoplar tudo a nmcli/GTK. |
| P1 | Primeira cobertura de uso comum e plataformas principais. |
| P2 | Ampliação de fabricantes, redes mesh e recursos específicos. |
| P3 | Pesquisa, legado, nichos ou integrações sem interface pública comprovada. |
| S / M / L / XL | Estimativa relativa: pequena, média, grande ou múltiplas tarefas. Não representa prazo prometido. |
| Estado inicial | Todos os itens das tabelas abaixo estão **A fazer**. Estudos podem terminar como inviáveis, com evidência. |

Cada item deve virar uma issue/PR com responsável, sistema e versão, dependências, cenário de teste e resultado. As tabelas são o backlog de referência; este documento não cria automaticamente dezenas de issues no GitHub.

## Primeira fila de execução

- [ ] **CORE-01:** separar o núcleo Rust da interface e do backend Linux.
- [ ] **CORE-02:** criar contrato de adaptadores e descoberta de capacidades.
- [ ] **CORE-03:** substituir o estado binário por estados de conexão/autenticação.
- [ ] **CORE-04:** definir formato de perfil versionado e migração.
- [ ] **AUTH-01:** integrar armazenamento seguro e segredos por backend.
- [ ] **AUTH-02:** definir desafios de login, MFA e navegador externo.
- [ ] **QA-01:** montar laboratório de túneis, começando por OpenVPN/WireGuard.
- [ ] **VPN-01 / VPN-02:** completar e homologar os dois protocolos existentes.
- [ ] **OS-00:** provar o núcleo e comparar opções de interface por plataforma.
- [ ] **FORT-01:** validar Fortinet SSL-VPN PPP em gateway de teste autorizado.
- [ ] **FORT-02:** levantar e validar separadamente Fortinet IPsec/SAML.

A ordem considera dependências; não impede estudos de Windows, Apple e Android em paralelo. Nenhum desses estudos deve interromper a versão Linux existente.

## 1. Núcleo e backends

| ID | Prioridade / esforço | Dependências | Entrega e aceite |
| --- | --- | --- | --- |
| CORE-01 | P0 / L | — | Separar modelos, validação, eventos e regras em crate Rust sem GTK/nmcli. Compilar o núcleo em Linux, Windows, macOS, Android e iOS; preservar os fluxos Linux. |
| CORE-02 | P0 / L | CORE-01 | Contrato de backend para descobrir, criar/importar, editar, conectar, cancelar, desconectar, exportar e consultar estado. Capacidades e erros tipados por protocolo e plataforma; operação indisponível não vira botão que falha silenciosamente. |
| CORE-03 | P0 / M | CORE-01 | Estados desconectado, preparando, autenticando, conectando, conectado, reconectando, encerrando e erro. Cancelamento determinístico, eventos sem polling contínuo e nenhuma indicação de “conectado” antes da confirmação do backend. |
| CORE-04 | P0 / M | CORE-01 | Perfil versionado com campos comuns e extensões específicas. Validar importação, caminho de certificados e migração sem perder perfis existentes. Segredos ficam fora do arquivo público de perfil. |
| CORE-05 | P0 / L | CORE-02 | Separar UI e operações privilegiadas. Polkit/serviço/extension conforme OS; IPC com autenticação e acesso mínimo. GUI não exige execução permanente como administrador. |
| CORE-06 | P1 / M | CORE-02 | Descobrir engines, plugins, versões e recursos realmente instalados. Detectar `vpn.service-type` no Linux em vez de tratar todo `vpn` como OpenVPN. Orientar instalação pelo canal do sistema, sem baixar e executar binários arbitrários. |

## 2. Autenticação e rede

| ID | Prioridade / esforço | Dependências | Entrega e aceite |
| --- | --- | --- | --- |
| AUTH-01 | P0 / L | CORE-01, CORE-05 | Cofre nativo por OS e escolha explícita de lembrar credenciais. Mapear segredos esperados por cada backend, sem assumir `vpn.secrets.password`. Cancelar/expirar login elimina material transitório; logs e argumentos de processo não contêm segredos. |
| AUTH-02 | P1 / XL | AUTH-01, CORE-03 | Contrato de desafios: OTP/PIN, troca de senha, grupos, cookies e login no navegador SAML/OIDC quando exposto pelo backend. Testar sucesso, negação, timeout e cancelamento. Não tratar MFA como uma segunda senha fixa. |
| AUTH-03 | P1 / L | AUTH-01 | Certificado CA/cliente, chave criptografada, PKCS#12 e pesquisa de PKCS#11/smart card. Seleção de arquivos, validação de identidade e erros claros; homologar somente os formatos efetivamente integrados. |
| NET-01 | P0 / L | CORE-02 | Rotas IPv4/IPv6, DNS por domínio e túnel dividido com leitura, aplicação e restauração. Não substituir opções do outro protocolo nem apagar configurações de redes alheias. Testes reais de resolução e tráfego. |
| NET-02 | P2 / XL | NET-01, CORE-05 | Kill switch opcional por adaptador/OS, se implementável. Testar queda de túnel, falha do processo, reinício e recuperação; regra de firewall tem escopo e reversão conhecidos. Não prometer garantia em sistemas sem API apropriada. |
| NET-03 | P1 / L | NET-01, CORE-03 | Reconexão com backoff, troca de Wi-Fi/dados, suspensão e retomada. Respeitar novo desafio de autenticação; nunca repetir credencial expirada indefinidamente. |
| NET-04 | P1 / M | CORE-02 | Diagnóstico com estado do backend, DNS, rotas, versão e erro acionável. Exportação de relatório sanitizado; não incluir configuração corporativa ou dados de login por padrão. |

## 3. Experiência e gestão dos perfis

| ID | Prioridade / esforço | Dependências | Entrega e aceite |
| --- | --- | --- | --- |
| UX-01 | P1 / M | CORE-02, CORE-04 | Adicionar por arquivo, QR code compatível ou assistente. Detectar conteúdo/assinatura do formato; extensão `.conf` não implica WireGuard. Mostrar o protocolo detectado e oferecer escolha quando houver ambiguidade. |
| UX-02 | P1 / M | CORE-03, AUTH-02 | Lista única, conectar/desconectar e configurações adaptadas ao perfil. Tratar botão ocupado, cancelamento, teclas, acessibilidade, textos longos e escala de fonte. Manter recursos técnicos em configurações, sem transformar a tela inicial em dashboard. |

## 4. Sistemas operacionais

Os limites e fontes de cada plataforma estão em [PLATFORMS.md](docs/PLATFORMS.md).

| ID | Prioridade / esforço | Dependências | Entrega e aceite |
| --- | --- | --- | --- |
| OS-00 | P0 / L | CORE-01, CORE-02 | Prova de conceito do núcleo, FFI e UI. Comparar frontends nativos/compartilhados por consumo, acessibilidade e manutenção. Registrar decisão; preservar GTK/Linux até existir substituto validado. |
| OS-01 | P1 / L | CORE-02, CORE-05, AUTH-01 | Adaptador NetworkManager com eventos e agente de segredos. Homologar inicialmente família Debian/Ubuntu, Fedora e Arch/CachyOS, KDE/GNOME e Wayland/X11 em matriz explícita. |
| OS-02 | P1 / XL | OS-00, CORE-05 | Windows: GUI/tray, serviço, permissões, instalação e remoção. Engines OpenVPN/WireGuard e API nativa IKEv2 em entregas distintas; validar x64/ARM64 e versões suportadas escolhidas. |
| OS-03 | P1 / XL | OS-00, CORE-05 | macOS: app, menu, cofre, integração de túnel e assinatura/notarização. Confirmar entitlements, distribuição e engines; testar arquiteturas e versões que a release declarar. |
| OS-04 | P1 / XL | OS-00, AUTH-01 | Android: bridge Rust, VpnService/APIs permitidas, consentimento, notificação e ciclo de vida. Importação móvel, QR, troca de rede e acesso por perfil de trabalho quando disponível. Validar em aparelho real. |
| OS-05 | P1 / XL | OS-00, AUTH-01 | iOS/iPadOS: bridge, extensão de túnel, cofre e distribuição. Resolver conta/entitlements/regras de publicação antes de anunciar release. Testar dispositivo real e limites do sistema, sem presumir acesso a perfis de outros apps. |
| OS-06 | P2 / L | CORE-02, CORE-05 | CLI/serviço Linux sem sessão gráfica ou NetworkManager. Escolher adaptadores explícitos; estado e configuração funcionam sem iniciar GUI. |
| OS-07 | P2 / M | OS-04 | ChromeOS: prova em dispositivos elegíveis e escopo da VPN no host. Distinguir app Android, recursos integrados e rede do contêiner Linux. Documentar indisponibilidade onde necessário. |
| OS-08 | P3 / XL | CORE-02 | FreeBSD primeiro; OpenBSD/NetBSD por estudos separados. Escolher engines e adaptar serviço, cofre, rotas/DNS e pacotes; build isolado não conta como homologação. |
| OS-09 | P3 / XL | OS-06 | OpenWrt/roteadores/NAS: CLI/serviço ou API do fabricante, conforme dispositivo. Validar arquitetura, armazenamento e capacidade de atualização; não exigir ambiente desktop. |
| OS-10 | P3 / L | OS-06, CORE-05 | Painel web opcional de agente autorizado. Autenticação, pareamento, proteção de origem/CSRF e indicação do host controlado. Sem porta remota aberta por padrão nem alegação de VPN do sistema feita só pelo navegador. |
| OS-11 | P3 / M | OS-00 | Levantamento para TVs, wearables, consoles e outros sistemas. Registrar API disponível, canal de distribuição e solução viável: app, controlador, gateway ou indisponível. Não prometer execução universal. |

## 5. Protocolos principais

| ID | Prioridade / esforço | Dependências | Entrega e aceite |
| --- | --- | --- | --- |
| VPN-01 | P1 / L | CORE-02, AUTH-01, AUTH-03 | Completar OpenVPN: importação fiel, certificados/chaves, MFA e exportação quando suportada. Definir política de scripts e opções de arquivo; não executar conteúdo importado sem controle. Homologar engine e opções por plataforma. |
| VPN-02 | P1 / L | CORE-02, NET-01 | Completar WireGuard: criar/editar peers, chaves, PSK, AllowedIPs, keepalive, importação/exportação e QR. Exportar chave privada somente por ação explícita; testes com engine apropriada por OS. |
| VPN-03 | P1 / XL | CORE-02, AUTH-03 | IKEv2/IPsec: backend Linux e APIs nativas em fases por sistema. Validar EAP/certificados e PSK quando oferecido; testar identidade do gateway e roaming. |
| VPN-04 | P2 / L | CORE-02, AUTH-01 | L2TP/IPsec: configuração, autenticação, MTU e tratamento de incompatibilidade. Documentar sistemas/versões sem backend permitido; não anunciar L2TP sem proteção como equivalente seguro. |

## 6. FortiClient/Fortinet — frente dedicada

Contexto e referências: [inventário Fortinet](docs/VPN-COMPATIBILITY.md#forticlient-resposta-específica).

| ID | Prioridade / esforço | Dependências | Entrega e aceite |
| --- | --- | --- | --- |
| FORT-01 | P1 / L | CORE-02, AUTH-02 | Fortinet SSL-VPN PPP: identificar variante e usar backend compatível; perfil com servidor, porta, realm/grupo quando aplicável, CA e MFA. Homologar gateway autorizado, login e reconexão. Não inferir suporte a outros modos Fortinet. |
| FORT-02 | P1 / XL | VPN-03, AUTH-02 | Fortinet IPsec: levantar IKE, autenticação e SAML por versão de FortiOS/FortiClient e sistema. Usar protocolo aberto somente quando a combinação for compatível; registrar necessidade de cliente oficial nos demais casos. |
| FORT-03 | P2 / XL | FORT-01, FORT-02 | Avaliar EMS, ZTNA, políticas de endpoint e integração oficial. Se exigirem SDK/cliente indisponível, declarar limitação ou integração delegada. Não simular aprovação de postura do dispositivo. |

## 7. Outros gateways empresariais

| ID | Prioridade / esforço | Dependências | Entrega e aceite |
| --- | --- | --- | --- |
| CORP-01 | P1 / L | CORE-02, AUTH-02 | Adaptador OpenConnect e primeira homologação Cisco AnyConnect/Secure Client/ocserv compatível. Grupos, certificado e desafios de login; distinguir execução direta de recursos oferecidos pelo plugin do sistema. |
| CORP-02 | P2 / L | CORP-01 | GlobalProtect: portal, descoberta/seleção de gateway e login. Testar políticas e diferenças de gateway; identificar cenários que exigem cliente oficial. |
| CORP-03 | P2 / XL | CORP-01 | Pacotes de homologação independentes para Pulse/Ivanti, Juniper NC, F5 e Array. Não liberar quatro marcas com base em um único teste OpenConnect. |
| CORP-04 | P2 / XL | CORE-02, AUTH-02 | Estudos separados de Check Point/SNX e SonicWall/NetExtender. Confirmar interface e autorização de automação; documentar sistemas e versões, incluindo quando apenas delegar ao cliente oficial. |
| CORP-05 | P3 / M | CORE-02 | Fila contínua para fabricantes, incluindo Sophos Connect e WatchGuard: modelo, versão, modo, autenticação, interface oficial e laboratório. Priorizar por demanda e viabilidade antes de escrever um conector. |
| CLOUD-01 | P2 / L | VPN-01, VPN-03, AUTH-02 | AWS Client VPN e Azure P2S: validar separadamente perfil, protocolo e autenticação. Registrar casos que exigem cliente oficial, como SAML conforme o produto; incluir conectividade e autorização às redes, não apenas login. VPNs site-to-site de nuvem entram em EDGE-02. |

## 8. Mesh, provedores e integrações especializadas

| ID | Prioridade / esforço | Dependências | Entrega e aceite |
| --- | --- | --- | --- |
| MESH-01 | P2 / L | CORE-02, AUTH-02 | Tailscale e estudo Headscale: registro, login, serviço local, peers e estado. Validar controle e políticas; cliente externo no mobile não equivale a engine incorporada. |
| MESH-02 | P2 / L | CORE-02, AUTH-02 | NetBird: registro autorizado, instância, serviço e estado. Testar configuração de rede e ciclo de vida, sem reutilizar perfil WireGuard como substituto da integração. |
| MESH-03 | P2 / L | CORE-02 | ZeroTier: adesão à rede, autorização, saída e estado. Avaliar API local e distribuição antes de incorporar componentes. |
| MESH-04 | P3 / XL | CORE-02, AUTH-03 | Estudos e adaptadores separados para Nebula, tinc e n2n. Gerenciar identidades/configurações conforme cada engine e registrar manutenção/versões. |
| MESH-05 | P3 / M | CORE-02 | Catálogo extensível de outros produtos overlay/ZTNA. Para cada um, diferenciar rede IP, acesso a aplicativo e controle de cliente oficial. |
| PROV-01 | P2 / M | VPN-01, VPN-02 | Importação assistida de perfis autorizados de provedores comerciais. Não solicitar senha do site quando o serviço usa credenciais de VPN distintas. Validar formatos e termos por integração. |
| PROV-02 | P3 / XL | CORE-02, AUTH-02 | Pesquisar NordLynx, NordWhisper, Proton Stealth e outros modos exclusivos. Verificar API, engine, licença, autenticação e distribuição; sem prometer compatibilidade a partir de semelhança com um protocolo aberto. |
| PROV-03 | P2 / L | CORE-02, AUTH-02 | WARP/Cloudflare One: interface oficial, registro, organização quando aplicável, estado e modos de operação. Distinguir DNS-only de túnel e transportes suportados. |
| ADV-01 | P3 / L | CORE-02 | SoftEther nativo: avaliar cliente próprio e licenciamento. Homologar o modo utilizado; conectar ao modo OpenVPN do servidor não comprova suporte ao protocolo nativo. |
| ADV-02 | P3 / L | CORE-02, AUTH-03 | Estudos independentes de OpenSSH TUN e Iodine. Diferenciar encapsulamento, autenticação e criptografia; limitar oferta aos cenários efetivamente testados. |
| ADV-03 | P3 / XL | CORE-02, AUTH-02 | Estudos separados de AmneziaWG, Lightway e MASQUE/CONNECT-IP. Documentar versão, API e formato de cada um; nenhuma engine nova entra sem laboratório e avaliação de distribuição. |
| LEG-01 | P3 / L | CORE-06 | IKEv1/Cisco IPsec legado, SSTP e PPTP: avaliar dependências e plataformas. Oferecer legado só por escolha explícita, com estado de manutenção visível e sem fallback criptográfico silencioso. |
| PROXY-01 | P3 / XL | CORE-02, NET-01 | Investigar SOCKS/HTTP CONNECT, SSH proxy, Shadowsocks, VMess/VLESS, Trojan, Hysteria e TUIC como categoria separada. Documentar alcance do tráfego e necessidade de TUN; proxy de um app não recebe selo de VPN completa. |
| EDGE-02 | P3 / M | OS-09 | Delimitar infraestrutura MPLS/L2VPN/L3VPN, EVPN/VXLAN, GRE e túneis L2. Decidir se há caso para configuração/exportação/controle de gateway; não tratá-los automaticamente como clientes VPN com criptografia. |

## 9. Qualidade, distribuição e continuidade

| ID | Prioridade / esforço | Dependências | Entrega e aceite |
| --- | --- | --- | --- |
| QA-01 | P0 / L | CORE-01 | Laboratório reproduzível de OpenVPN, WireGuard e IKEv2; fixtures sintéticas e integração de backends. Gateways comerciais só com contas/appliances autorizados. Nenhum dado empresarial real no repo ou CI público. |
| QA-02 | P1 / L | QA-01, OS-00 | Matriz automatizada e manual protocolo × OS × engine × autenticação × gateway. Cobrir tráfego, DNS/IPv6, falha, suspensão, troca de rede, cancelamento e rollback; registrar versões e evidências sanitizadas. |
| QA-03 | P1 / M | QA-02 | Medir UI e engine separadamente: RAM, CPU, bateria e despertar em repouso/conexão. Definir orçamento por plataforma e detectar regressões; sem impor a medição Linux como meta universal. |
| QA-04 | P1 / M | CORE-04, AUTH-01 | Validar perfis malformados, caminhos, arquivos externos e comportamento de logs/exportação. Revisar dependências e rastrear vulnerabilidades que afetam as engines empacotadas. |
| REL-01 | P0 / M | — | Definir licença do app e inventário de componentes. Verificar compatibilidade de integração/redistribuição e quais exigem instalação externa; registrar conclusão antes do empacotamento. |
| REL-02 | P1 / XL | QA-02, REL-01 | CI e pacotes por plataforma, assinaturas e credenciais de publicação protegidas. Linux primeiro; instaladores Windows/macOS e canais móveis em etapas. Listar suporte real de Flatpak/Snap, sem presumir acesso ao serviço do host. |
| REL-03 | P1 / M | REL-02 | Publicar matriz de suporte, notas e atualização com verificação de integridade, migração e reversão. Rever fontes e dependências por release; retirar selo homologado se uma atualização invalidar o teste. |

## Marcos de entrega

| Marco | Resultado esperado | Condição para concluir |
| --- | --- | --- |
| M0 — Base extensível | Núcleo Rust, contrato de backend, estados, perfis e diagnóstico de capacidades. | CORE-01–05, AUTH-01, NET-01, QA-01 e decisões REL-01/OS-00 concluídos. |
| M1 — Linux homologado | OpenVPN/WireGuard completos; início de IKEv2 e Fortinet. | Testes de túneis reais e matriz publicada para combinações entregues. Fortinet só recebe suporte depois de seu próprio aceite. |
| M2 — Desktop multiplataforma | Linux, Windows e macOS com interface simples e engines testadas. | Pacotes e cenários homologados por sistema; sem exigir paridade de todos os protocolos no primeiro release. |
| M3 — Mobile | Android e iOS/iPadOS com serviços/extensões e ciclo de vida próprios. | Dispositivos reais, autenticação, consumo e distribuição resolvidos. Bloqueios de publicação ficam visíveis. |
| M4 — Integrações ampliadas | Outros fabricantes, mesh, provedores e recursos de rede. | Cada conector passa por QA-02; integrações delegadas continuam identificadas como tal. |
| M5 — Plataformas adicionais | BSD, ChromeOS e embedded conforme viabilidade. | Artefatos, manutenção e matriz próprios; estudos inviáveis registrados, sem promessa de universalidade. |

## Definição de pronto para uma integração

- [ ] Capacidades e modo — próprio ou delegado — visíveis na interface.
- [ ] Perfil e autenticação suportados documentados, sem campos genéricos indevidos.
- [ ] Instalação, autorização, login, tráfego real e desligamento verificados.
- [ ] DNS, IPv6, rotas, cancelamento, falha e recuperação verificados no escopo oferecido.
- [ ] Sem perda de configurações externas ou exposição de credenciais.
- [ ] Pacote/distribuição e licença compatíveis com o sistema-alvo.
- [ ] Consumo medido e limitações publicadas com versões, data e evidência.

## Dependências externas a obter

Appliances/contas de teste de fabricantes, máquinas Windows/macOS, dispositivos Android/Apple, acesso a assinatura e canais de distribuição e decisões de licenciamento. Documentar o bloqueio na tarefa correspondente; não trocar uma dependência ausente por uma afirmação de compatibilidade.

Não há previsão de prazo ou custo nesta pesquisa. Antes de iniciar cada item XL, dividi-lo em entregas por backend, plataforma e autenticação, com estimativa depois do protótipo.
