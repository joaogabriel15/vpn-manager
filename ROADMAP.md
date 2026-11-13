# Roadmap do VPN Manager

Atualizado em **12/11/2026**. Escopo atual: **Linux, Windows e macOS**. Android e iOS/iPadOS ficam para uma fase futura.

Linux mantém GTK/NetworkManager. Windows/macOS agora têm interface e CLI iniciais para perfis VPN do sistema, com atualização manual e autenticação configurada no próprio sistema. O workflow empacota os quatro targets; builds nativos remotos e túneis reais ainda precisam de evidência. Veja [as releases desktop](docs/DESKTOP-RELEASES.md).

## Visão geral

| Etapa | Resultado esperado | Estado |
| --- | --- | --- |
| 1. Base desktop compartilhada | Modelos, perfis, validação, estados, contratos e eventos em Rust | Em andamento; app Linux já usa o núcleo |
| 2. Linux completo | OpenVPN/WireGuard pelo app, autenticação, reconexão, DNS/IPv6 e instalação validados | Em andamento; testes de engines e integração sintética passaram |
| 3. Windows | Aplicativo com bandeja, backend/serviço, cofre e instalador | Inicial; interface/CLI e adaptador de perfis nativos, sem homologação de túnel |
| 4. macOS | Aplicativo com menu, mecanismo de túnel, cofre e distribuição assinada | Inicial; interface/CLI e adaptador de perfis nativos, sem homologação de túnel |
| 5. Mais protocolos e gateways | IKEv2 e conectores empresariais homologados por combinação | Planejado; Fortinet tem cenários separados |
| 6. Releases desktop | Pacotes reproduzíveis, matriz de suporte, atualização e desinstalação | Workflow de pacotes e prereleases por tag implementado; execução remota pendente |

O trabalho compartilhado atende aos três desktops. Provas de conceito de Windows e macOS podem avançar enquanto o Linux é concluído. Cada release depende de sua própria validação. Não há datas de entrega definidas.

## 1. Base desktop compartilhada

- [x] Separar interface GTK, núcleo Rust e adaptador NetworkManager em crates.
- [x] Definir contrato de gestão de perfis, conexão, cancelamento e eventos.
- [x] Adicionar estados explícitos e erros tipados.
- [x] Criar TOML v2 para referências e OpenVPN básico, com migração das referências v1.
- [x] Validar campos e remover o perfil recém-criado quando a aplicação de rede falha.
- [x] Adicionar CI para núcleo nativo em Linux/Windows/macOS e checks dos targets desktop.
- [ ] Validar a execução da CI nos três sistemas.
- [ ] Concluir capacidades por protocolo, estados de autenticação e contratos de desafios.
- [ ] Ampliar o perfil versionado para as configurações adicionais necessárias.
- [ ] Definir a estratégia de interface e a fronteira entre GUI e operações privilegiadas de cada desktop.

Referências: CORE-01–06, AUTH-02 e OS-00 no [backlog](BACKLOG.md).

## 2. Concluir o Linux

- [x] Interface com busca, bandeja, importação, criação, edição, duplicação, exportação compatível e remoção.
- [x] Identificar o protocolo do perfil e consultar plugins instalados.
- [x] Atualizar por eventos, sem consultas periódicas de estado em repouso.
- [x] Ler e testar os estados específicos de VPN por D-Bus, incluindo preparação, autenticação e falha, além de validar assinatura e encerramento dos sinais em barramento privado.
- [x] Implementar cancelamento e testar ativação tardia com adaptador simulado.
- [x] Adicionar cofre opcional Secret Service, com salvamento padrão da senha OpenVPN e opção de uso único.
- [x] Transmitir a senha OpenVPN por descritor de arquivo anônimo 0600.
- [x] Preservar opções independentes de IPv4 e IPv6 no editor.
- [x] Testar túneis OpenVPN/WireGuard isolados com IPv4, IPv6, DNS e desligamento.
- [x] Medir uma amostra de CPU e memória da GUI em repouso.
- [ ] Testar o fluxo completo **aplicativo → NetworkManager → gateway** em laboratório.
- [ ] Homologar cofre real, senha, certificados e cancelamento durante autenticação.
- [ ] Implementar agente de segredos e desafios MFA/SAML no escopo oferecido pelo backend.
- [ ] Implementar e validar reconexão, suspensão/retomada e troca de rede.
- [ ] Verificar restauração de DNS/rotas e tráfego IPv4/IPv6 após falhas.
- [ ] Homologar instalação, atualização e remoção nas distribuições e ambientes declarados.

Critério de conclusão: os fluxos acima passam pelo app, com versões e evidências registradas; o laboratório das engines sozinho não encerra esta etapa. Resultados atuais em [VALIDATION.md](docs/VALIDATION.md).

## 3. Implementar Windows

- [x] Interface/CLI para listar e solicitar conexão/desligamento de perfis VPN do usuário via ferramentas Windows; pacote ZIP portátil.
- [x] Teste sintético de descoberta e limpeza de perfil na CI, sem ativar túnel (execução remota pendente).

- [ ] Provar o adaptador e a integração autorizada com engines OpenVPN/WireGuard.
- [ ] Implementar serviço e comunicação autenticada com privilégios mínimos.
- [ ] Integrar cofre, eventos, cancelamento, DNS/rotas e reconexão.
- [ ] Entregar interface simples e bandeja usando as capacidades reais do backend.
- [ ] Produzir instalador, atualização e remoção; resolver assinatura e licenças de componentes.
- [ ] Testar login, tráfego, IPv6/DNS, queda do serviço e ciclo de vida em Windows real.
- [ ] Publicar as versões e arquiteturas efetivamente homologadas.

Referências: OS-02, CORE-05, AUTH-01, NET-01/03 e REL-01–03. Interface inicial eframe/egui; consumo e acessibilidade nativos ainda precisam de medição.

## 4. Implementar macOS

- [x] Interface/CLI para perfis VPN expostos por `scutil --nc`, com pacote `.app` para Intel e Apple Silicon e assinatura ad hoc.

- [ ] Provar o adaptador e escolher o mecanismo de túnel e a distribuição permitidos para cada protocolo.
- [ ] Definir integração entre núcleo Rust, aplicação e componentes do sistema.
- [ ] Integrar cofre, eventos, cancelamento, DNS/rotas e reconexão.
- [ ] Entregar interface e menu com comportamento adequado ao macOS.
- [ ] Resolver assinatura, permissões, notarização e instalação/remoção conforme o canal escolhido.
- [ ] Testar login, tráfego, IPv6/DNS, suspensão e recuperação em macOS real.
- [ ] Publicar as versões e arquiteturas efetivamente homologadas.

Referências: OS-03, CORE-05, AUTH-01, NET-01/03 e REL-01–03. Requisitos de distribuição precisam ser confirmados antes de anunciar pacotes.

## 5. Ampliar protocolos e gateways

- [ ] Adicionar IKEv2 ao laboratório e implementar os adaptadores desktop correspondentes.
- [ ] Homologar OpenConnect por gateway e método de autenticação.
- [ ] Tratar Fortinet SSL-VPN PPP, IPsec/SAML e políticas de endpoint como integrações distintas.
- [ ] Implementar recursos avançados, como kill switch opcional, somente onde houver suporte e testes de reversão.
- [ ] Priorizar os demais fabricantes, redes mesh e provedores conforme demanda e viabilidade.

Cada combinação de protocolo, engine, sistema, autenticação e gateway precisa de evidência própria. Plugin instalado ou protocolo semelhante não significa suporte homologado ao produto. Consulte o [inventário](docs/VPN-COMPATIBILITY.md).

## 6. Distribuir e manter

- [x] Automatizar pacotes Linux/Windows/macOS e checksums; tags publicam prereleases após a matriz completa passar. Execução remota pendente.

- [ ] Definir licença do projeto e inventário de licenças das dependências/engines.
- [ ] Publicar matriz de suporte com limitações e testes reproduzíveis.
- [ ] Criar pacotes desktop, assinaturas e verificação de integridade.
- [ ] Validar atualização, migração e desinstalação preservando os perfis do usuário.
- [ ] Definir limites de consumo por plataforma e acompanhar regressões.

## Fase futura

Android e iOS/iPadOS, suas bridges, serviços/extensões e distribuição ficam adiados. BSD, ChromeOS, dispositivos embarcados e painel web também permanecem fora da entrega desktop atual. O planejamento está preservado no [backlog detalhado](BACKLOG.md) e no [plano de plataformas](docs/PLATFORMS.md).
