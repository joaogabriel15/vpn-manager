# Validação local — 12–13/11/2026

Os resultados abaixo se referem ao código local desta etapa. Não constituem homologação de fabricantes ou aplicativos para outros sistemas.

| Escopo | Resultado observado | Limite |
| --- | --- | --- |
| Workspace Rust | 43 testes passaram, incluindo três testes de integração com NetworkManager | Perfis sintéticos; sem ativar VPNs do host |
| Contrato Linux | Criar, ler, editar, duplicar, exportar e remover; preservação IPv4/IPv6; rollback de criação quando a rede falha | Servidores reais não fazem parte desses testes |
| Cancelamento | Antes de iniciar, ativação tardia simulada e falha ao desligar | Testes do contrato e cancelamento de perfil inativo; interrupção de autenticação real ainda pendente |
| Credenciais transitórias | Descritor herdado de arquivo anônimo 0600; senha fora dos argumentos; nenhum arquivo nomeado para abandonar numa queda da GUI | O cliente mantém seu descritor até encerrar; o nmcli tem timeout de 40 segundos |
| Cofre | Salvar, consultar e esquecer via provedor sintético compatível com secret-tool; erros não expõem saída do provedor | Não foi alterado o cofre real do usuário; desbloqueio e integração com provedores reais ainda precisam de homologação |
| Interface GTK | `UI_SMOKE_OK`: consulta, busca, bandeja, editor e cancelamento de login com limpeza de senha e opção de uso único desmarcada quando há cofre | Sem login, gravação no cofre ou túnel real pelo app |
| Núcleo portátil | `cargo check` em Linux x64, Windows GNU x64, macOS ARM64, Android ARM64 e iOS ARM64 | Revalidar após mudanças; isso não produz aplicativos, serviços ou pacotes nativos |
| Estados D-Bus | Preparação, autenticação, conexão, falha e estados desconhecidos; sinais atualizam o backend e param após desinscrição | `python3 tests/states.py` usa serviço e barramento privados; falta homologar autenticação real |
| Qualidade | Formatação, Clippy com warnings como erros e build release | Workflows adicionados; execução remota não observada |

## Reutilização automática de senhas — 13/11/2026

`python3 tests/saved-login.py`: verifica login GTK com uso único sem gravação e login seguinte com salvamento padrão após sucesso e uma segunda conexão usando a senha salva, sem formulário nem redigitação. Uma tentativa posterior rejeitada voltou ao formulário e foi cancelada sem modificar o item salvo (`SAVED_LOGIN_OK`). Secret Service, nmcli e D-Bus são fixtures isoladas; a fixture confere a senha recebida pelo descritor anônimo e sua ausência nos argumentos. O cofre e as VPNs reais do host não são usados. A fixture confirma exatamente uma gravação: uso único, reutilização e cancelamento não gravam. O teste de provedor também verifica isolamento entre identificadores de perfis/contas.

O teste gráfico comum continua sem consultar o cofre real. Homologação de desbloqueio, políticas e persistência em provedores reais continua pendente.

## Ampliação de plugins Linux — 13/11/2026

- Integração com perfis inativos de OpenConnect, fortisslvpn, strongSwan, Libreswan, L2TP, SSTP, VPNC, PPTP, SSH, Iodine e um serviço sintético ausente. Protocolo, estado desconectado, leitura/alteração de DNS e remoção passaram; conexão do plugin ausente recusada antes de ativar. Não comprova engines, autenticação ou túneis dessas famílias.
- Sanitização PCF reconstrói a seção principal sem senhas claras/codificadas, incluindo chaves duplicadas, e desativa salvamento antes de chamar o plugin; teste sintético passou. Importação real VPNC pendente porque o plugin não está instalado no host.
- Detecção de conteúdo misto, hooks/includes executáveis, nomes de serviços, argumentos do editor/terminal e resolução de bibliotecas/helpers relativos testados sem executar plugins.
- Exportação OpenVPN padrão permanece aceita pela política de importação; tentativa de sobrescrever um arquivo exportado é rejeitada e preserva os bytes originais.
- Corrigida corrida quando outro cliente remove um perfil entre a listagem e a consulta de detalhes. A fixture read-only de D-Bus/interface simula o desaparecimento sem revelar dados reais.
- As novas rotas de configuração/autenticação são delegadas a editores e agentes do sistema. Cancelamento de formulários é verificado sem abrir editores ou iniciar terminais. Não há garantia de MFA/SAML, restauração, reconexão ou compatibilidade universal.

## Desktop e releases 0.2.0

- Testes da GUI sem janela verificam que erro/interrupção remove indicação antiga de conectado e que operação pendente bloqueia solicitações duplicadas. Dois testes adicionais passaram com `--features gui`.
- Testes dos adaptadores: inventários Windows/macOS sintéticos, Unicode, estados desconhecidos, rejeição de identidades inválidas, argumentos sem avaliação de shell, timeout e erros sem dados privados. Os processos são testados no Linux; não houve execução local de ferramentas Windows/macOS.
- `cargo check -p vpn-manager --target x86_64-pc-windows-gnu --locked` , `--target x86_64-pc-windows-msvc`, `--target x86_64-apple-darwin` e `--target aarch64-apple-darwin`: aplicativo completo verificado, incluindo GUI e adaptador, sem GTK. Isso não prova linkedição MSVC/macOS nem execução nativa.
- `python3 tests/release-packaging.py`: quatro testes cobrem layout/permissões Linux, layout Windows, checksum, conjunto completo, corrupção, binário ausente, recusa de sobrescrita e versão inválida. Os arquivos usados nesses testes são sintéticos, sem alegação de execução dos binários.
- Workflow nativo testa/builda Windows MSVC, macOS Intel/ARM e Linux; teste Windows cadastra/remove perfil fictício sem conectar. Execução remota e assinatura ad hoc macOS ainda não observadas. Windows/macOS não estão homologados para tráfego, DNS, IPv6, MFA ou recuperação.
- Pacote Linux real gerado localmente; extração, instalação em HOME temporário, launcher, CLI e inclusão das licenças das fontes passaram (`RELEASE_INSTALL_OK`). Smoke GTK e estados D-Bus passaram após separar o entrypoint por OS. O pacote local usa as bibliotecas do host de desenvolvimento; a base Ubuntu 24.04 é aplicada pelo workflow.
- Workflows verificados com Actionlint, sem erros.
- O workflow publica somente prereleases, com notas de limites e SHA-256. Publicação não foi acionada nesta etapa.

## Validação de rotas no núcleo (NET-01 / QA-04)

Destino IP e prefixo (0–32 ou 0–128), família do gateway e métrica u32 são validados antes de modificar o perfil ou criar a configuração TOML. Mensagens de erro não reproduzem os valores fornecidos. Os testes cobrem rotas múltiplas, rota padrão, host, limpeza e atributos opcionais. A integração com NetworkManager confirma que uma rota inválida impede também a alteração de DNS na mesma operação, e que limpar rotas IPv4 preserva IPv6. O rollback após criação continua exercitado com atributo rejeitado pelo daemon.

Sintaxe conferida na [documentação oficial do NetworkManager](https://networkmanager.pages.freedesktop.org/NetworkManager/NetworkManager/nm-settings-nmcli.html). Atributos específicos são encaminhados ao backend, que valida suporte e valores; a validação do núcleo não homologa tráfego nem restauração de rede.

## Túneis reais em laboratório isolado

Executado com Podman rootless, contêineres separados, rede interna sem portas publicadas, certificados/chaves sintéticos gerados a cada execução e limpeza ao terminar. Nenhum perfil, rota ou DNS do NetworkManager do host foi alterado.

| Cenário | Engine identificada | IPv4 | IPv6 | DNS pelo túnel | Desligamento |
| --- | --- | --- | --- | --- | --- |
| OpenVPN UDP com certificados de cliente e servidor | OpenVPN 2.6.14 / OpenSSL 3.0.22 | Passou | Passou | Passou | Passou |
| WireGuard entre peers sintéticos | wireguard-tools 1.0.20210914; backend kernel/userspace selecionado conforme disponibilidade | Passou | Passou | Passou | Passou |

Esses testes validam os cenários das engines. Não comprovam o fluxo completo GTK → NetworkManager → gateway, autenticação por senha/MFA, IKEv2, suspensão, troca de rede, restauração de DNS do host ou kill switch. O modo exato kernel/userspace do WireGuard não foi registrado nesta execução.

Reproduzir: `python3 tests/lab/run.py`. Requer Podman ou Docker, `/dev/net/tun` e permissão para `NET_ADMIN`/`NET_RAW` dentro dos contêineres. O build baixa Debian e pacotes; as versões podem receber atualizações. As chaves não entram na imagem nem são mantidas no repositório. O workflow manual `Isolated VPN engines` executa o mesmo laboratório.

## Consumo observado

`python3 tests/measure-idle.py`: janela GTK em repouso por 10 segundos após o smoke test, sem engine de túnel no processo. Resultado: **0,07 s de CPU**, **0,7% de um núcleo**, **75.880 KiB de RSS**. É uma amostra local, sem orçamento de regressão estabelecido; não mede bateria nem os clientes VPN externos.

## Pendências para ampliar a homologação

- Linux: túneis via NetworkManager em laboratório, cofre real, senha/MFA, agente próprio, reconexão, cancelamento sob autenticação e restauração de rede. IKEv2 ainda não foi adicionado ao laboratório.
- Windows/macOS: validar interface e adaptadores iniciais em máquinas nativas; implementar engines próprias, serviços/extensões, cofre, eventos, instaladores assinados e homologação de túneis.
- Android/iOS: adiados por decisão do proprietário. Os checks anteriores são histórico técnico, não requisito ou promessa da entrega desktop.
- Fortinet e outros fabricantes: gateway autorizado, versão, modo e autenticação definidos; testes separados para SSL-VPN, IPsec, SAML e postura de endpoint.
- Distribuição: decisão de licença, inventário de licenças e credenciais dos canais de publicação. Nenhum pacote foi publicado nesta etapa.

O [backlog](../BACKLOG.md) continua sendo a referência dos critérios completos de aceite.
