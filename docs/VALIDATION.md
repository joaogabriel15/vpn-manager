# Validação local — 12/11/2026

Os resultados abaixo se referem ao código local desta etapa. Não constituem homologação de fabricantes ou aplicativos para outros sistemas.

| Escopo | Resultado observado | Limite |
| --- | --- | --- |
| Workspace Rust | 27 testes passaram, incluindo dois testes de integração com NetworkManager | Perfis sintéticos; sem ativar VPNs do host |
| Contrato Linux | Criar, ler, editar, duplicar, exportar e remover; preservação IPv4/IPv6; rollback de criação quando a rede falha | Servidores reais não fazem parte desses testes |
| Cancelamento | Antes de iniciar, ativação tardia simulada e falha ao desligar | Testes do contrato e cancelamento de perfil inativo; interrupção de autenticação real ainda pendente |
| Credenciais transitórias | Descritor herdado de arquivo anônimo 0600; senha fora dos argumentos; nenhum arquivo nomeado para abandonar numa queda da GUI | O cliente mantém seu descritor até encerrar; o nmcli tem timeout de 40 segundos |
| Cofre | Salvar, consultar e esquecer via provedor sintético compatível com secret-tool; erros não expõem saída do provedor | Não foi alterado o cofre real do usuário; desbloqueio e integração com provedores reais ainda precisam de homologação |
| Interface GTK | `UI_SMOKE_OK`: consulta, busca, bandeja, editor e cancelamento de login com limpeza de senha e armazenamento desmarcado | Sem login, gravação no cofre ou túnel real pelo app |
| Núcleo portátil | `cargo check` em Linux x64, Windows GNU x64, macOS ARM64, Android ARM64 e iOS ARM64 | Revalidar após mudanças; isso não produz aplicativos, serviços ou pacotes nativos |
| Estados D-Bus | Preparação, autenticação, conexão, falha e estados desconhecidos; sinais atualizam o backend e param após desinscrição | `python3 tests/states.py` usa serviço e barramento privados; falta homologar autenticação real |
| Qualidade | Formatação, Clippy com warnings como erros e build release | Workflows adicionados; execução remota não observada |

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
- Windows/macOS: adaptadores, serviços/extensões, interface, instaladores e máquinas para testes nativos.
- Android/iOS: adiados por decisão do proprietário. Os checks anteriores são histórico técnico, não requisito ou promessa da entrega desktop.
- Fortinet e outros fabricantes: gateway autorizado, versão, modo e autenticação definidos; testes separados para SSL-VPN, IPsec, SAML e postura de endpoint.
- Distribuição: decisão de licença, inventário de licenças e credenciais dos canais de publicação. Nenhum pacote foi publicado nesta etapa.

O [backlog](../BACKLOG.md) continua sendo a referência dos critérios completos de aceite.
