# Notas para retomar o desenvolvimento

Atualizado em 13/11/2026.

## Projeto e conta

- Projeto independente: `/home/pacheco/Projetos/vpn-manager`. Não pertence ao Excalibur.
- Repositório público: https://github.com/joaogabriel15/vpn-manager
- Conta GitHub a usar: **joaogabriel15**, conforme pedido explícito do proprietário.
- Antes de publicar: conferir `gh api user --jq .login` e `git remote get-url origin`; não usar outra conta autenticada.
- Git local configurado com e-mail noreply da conta pessoal.

## Pedido registrado

O pedido inicial foi pesquisar e planejar ampla cobertura de protocolos e sistemas, incluindo Fortinet. Depois, o proprietário autorizou implementar as pendências. **A orientação mais recente restringe o foco atual a Linux, Windows e macOS; Android e iOS/iPadOS ficam para o futuro.** Não iniciar trabalho móvel nesta fase e não tratar seus requisitos como bloqueio da entrega desktop.

## Onde está o planejamento

- [Roadmap desktop](../ROADMAP.md): etapas de entrega para Linux, Windows e macOS; mobile adiado.
- [Backlog priorizado](../BACKLOG.md): 61 itens, dependências, esforço relativo, critérios de aceite e marcos.
- [Inventário de VPNs](VPN-COMPATIBILITY.md): protocolos, fabricantes, Fortinet, redes mesh, provedores e pesquisas adjacentes; referências primárias.
- [Plano de plataformas](PLATFORMS.md): núcleo Rust compartilhado, adaptadores nativos, limites de cada sistema e requisitos de distribuição.

Nenhuma pesquisa deve ser apresentada como compatibilidade já entregue. Não há como garantir literalmente todos os produtos e versões; a estratégia é catálogo extensível e homologação por combinação.

## Estado entregue antes deste backlog

App Linux em Rust/GTK com NetworkManager, bandeja e instalação por usuário. Interface compacta: busca, lista de perfis, conectar e menus. Ícones do KDE corrigidos com identificação da aplicação e recursos de ícone adequados. Modais têm rodapé com margens; os botões reorganizam em largura reduzida. Fluxos OpenVPN/WireGuard existem, mas testes de interface e perfis temporários não homologam túneis reais.

Manter a interface simples e prática, sem painéis decorativos ou textos promocionais. Preservar consumo reduzido e atualização por eventos. O backlog não autoriza reescrever a UI antes de validar a arquitetura.

## Documentação e capturas

O README contém capturas reais da interface com dados sintéticos em `docs/screenshots/`, incluindo lista, editor, rede, opções avançadas, largura reduzida e login vazio. Reproduzir com `cargo build --release --locked` e `python3 tests/screenshots.py`. O script injeta `tests/fixtures/preview_nmcli.py`, que só responde consultas de demonstração e bloqueia mutações, e um serviço D-Bus em barramento privado; não usa perfis reais nem acessa o cofre. O login tem altura inicial de 480 px para acomodar os controles de cofre.

## Senhas salvas e reutilização — 13/11/2026

- OpenVPN: salvar é o padrão quando o cofre está disponível e ocorre só após conexão bem-sucedida. A opção Usar senha somente desta vez (não salvar) impede persistir/substituir a senha naquela tentativa; não apaga senhas antigas. Trocar o usuário ou carregar uma senha não desfaz essa escolha. Cliques seguintes em Conectar consultam o cofre por perfil/usuário e tentam conectar sem abrir o formulário. Falha abre o login para correção, sem loop; cancelamento não inicia outra tentativa.
- Menu Senha salva e login… força o formulário para trocar/carregar/esquecer. Esquecer limpa também o campo transitório; respostas assíncronas são ignoradas quando o usuário mudou/fechou o diálogo, e as ações de cofre não correm entre si.
- Outros plugins: atalho Salvar credenciais no sistema… abre o editor nativo e suas opções de cofre; nenhum mapeamento genérico de senha foi inventado.
- `python3 tests/saved-login.py` percorre o GTK real com cofre/nmcli/D-Bus sintéticos: verifica uso único sem salvar, salva por padrão, reconecta sem formulário e transmite a senha por FD; sem cofre/túnel reais. `--self-test` comum força o formulário e cancela sem consultar o cofre do usuário. CI Linux adiciona teste gráfico isolado com Xvfb.

## Ampliação Linux — 13/11/2026

- Catálogo `protocols.rs` com 12 famílias e nomes compartilhados. Plugin desconhecido continua identificado pelo serviço; não há fallback para OpenVPN.
- `desktop.rs` delega configuração a nm-connection-editor (`--create --type=vpn:<serviço>` / `--edit <uuid>`) ou ao módulo de redes KDE. A GUI permite Outros protocolos e fecha a edição interna antes de abrir a externa, evitando salvar campos desatualizados depois.
- Login por terminal nmcli --ask opcional na GUI e CLI, sem credenciais nos argumentos. Não equivale a agente de segredos próprio nem garantia de MFA/SAML. Estado do terminal não é confirmação do túnel.
- Inventário resolve helpers/bibliotecas relativos e verifica engine antes de conectar. Biblioteca presente não certifica import/export. CLI: catalog, import, validate-import, export, edit, create-native e connect <uuid> --ask.
- Detecta PCF por conteúdo e passa arquivo privado ao vpnc sem senhas/salvamento. A importação vpnc real ainda não foi testada: plugin ausente no host. Política rejeita hooks executáveis/includes OpenVPN e hooks WireGuard; aceita script-security 0–2 sem hooks para preservar exportação OpenVPN padrão.
- Exportação privada não sobrescreve arquivos. Descoberta ignora perfil removido por outro cliente entre list/show, sem derrubar a lista inteira. Fixture gráfica e de estados inclui esse cenário.
- Integração de perfis inativos de 11 serviços, incluindo plugin sintético ausente, passou com DNS/remoção e bloqueio de ativação sem engine. Sem login ou tráfego real desses gateways. Detalhes em [LINUX-VPN-SUPPORT.md](LINUX-VPN-SUPPORT.md).

## Releases e desktop 0.2.0

- `src/main.rs` seleciona a interface por OS; o app GTK original está em `src/linux.rs`. Dependências GTK/NetworkManager/bandeja são exclusivas do Linux.
- `vpn-manager-native` compartilha os modelos/estados/erros do núcleo. Windows: consulta fixa PowerShell Get-VpnConnection por usuário e rasdial com catálogo explícito do usuário. macOS: scutil --nc list/start/stop por UUID, filtrando PPP que não é VPN. Nenhum nome é interpolado em shell; saídas de erro são sanitizadas. Operações têm timeout e rodam fora da thread da GUI.
- GUI Windows/macOS eframe/egui com busca, atualizar, conectar/desconectar e configurações nativas; CLI `vpn-manager-native-cli` renomeada para `vpn-manager-cli` nos pacotes. Atualização manual; sucesso de comando significa solicitação, não conexão confirmada. Sem engines OpenVPN/WireGuard, agente/cofre próprio, bandeja ou contrato de eventos nessas plataformas. Ainda não implementa todo o contrato Backend de gestão do Linux.
- Workflow `release.yml`: execução manual gera artefatos; tags com versão igual ao Cargo.toml publicam prereleases após os quatro builds/checksums passarem. Linux tar.gz; Windows ZIP portátil; macOS .app em ZIP para Intel/ARM, assinatura ad hoc sem notarização. Inventário e textos de licenças acompanham os pacotes. Decisão da licença do app permanece pendente.
- Detalhes e comandos em [DESKTOP-RELEASES.md](DESKTOP-RELEASES.md). Não houve push de tag nem publicação observada nesta etapa.
- Testes locais dos parsers e processos e checks cruzados do app Windows GNU/MSVC e macOS Intel/ARM passaram; não equivalem a execução nativa ou túnel real. CI adiciona perfil fictício no Windows e o remove em finally, sem ativar. A execução remota desse teste ainda precisa ser observada.

## Próximo ponto técnico

Implementação em andamento em 12/11/2026:

- Workspace: GTK, `vpn-manager-core` e `vpn-manager-networkmanager` com GIO, sem GTK. Modelos, validação de campos, estados, erros, eventos e contrato de gestão estão no núcleo.
- Contrato Linux cobre descobrir, ler, editar, criar OpenVPN, importar, exportar, duplicar, remover, conectar, cancelar e consultar estado. A UI usa essas operações; diagnóstico de dispositivos permanece específico do Linux.
- `nmcli monitor` pertence ao adaptador, junto da assinatura D-Bus de `VpnStateChanged`; ambos são encerrados por Drop. Emite eventos tipados sem transmitir texto com nomes privados. Inventário consulta metadados `.name`, executáveis de serviço/autenticação e versão do nmcli; não executa binários de plugin para sondar capacidades.
- Cancelamento usa token e desligamento final depois de a ativação concluir, cobrindo ativação tardia. O botão existe somente durante a tentativa, não durante gravação posterior no cofre. Falha de desligamento é reportada.
- Validação de rotas no núcleo (NET-01/QA-04): destino/prefixo, família do gateway e métrica u32, inclusive no TOML. Limpeza de rotas permitida; atributos específicos continuam validados pelo backend. Integração verifica rejeição antes de modificar DNS e preservação de rotas IPv6 ao limpar IPv4. Rollback usa atributo não suportado pelo daemon para continuar cobrindo falha após criação.
- TOML v2: referências v1 migradas sem alterar backend/id; configurações básicas OpenVPN com CA absoluta e opções IPv4/IPv6. Backend valida existência do arquivo CA. Falha ao aplicar rede remove o perfil recém-criado. Extensões para outros protocolos e opções OpenVPN completas permanecem pendentes.
- Cofre opcional via Secret Service/secret-tool, com salvamento padrão após sucesso e opção explícita de uso único. Itens separados por perfil/usuário, controles para obter/esquecer, erros do provedor sanitizados. Testado com provedor sintético, não com o cofre real. GTK/GIO mantêm buffers próprios; os buffers de segredo do núcleo usam zeroize.
- Credenciais nmcli usam arquivo anônimo 0600 herdado por descritor no Linux, sem caminho de arquivo nomeado. Teste confirmou permissões e transmissão fora dos argumentos.
- CLI sem GTK: gestão básica, referências, validar/criar TOML, inventário e cancelamento. Instalador copia a CLI quando seu binário já foi compilado.
- Laboratório rootless em `tests/lab`: túneis OpenVPN e WireGuard com IPv4, IPv6, DNS e desligamento passaram. Cenário de engines, sem alterar conexões NetworkManager do host. Ainda não homologa o fluxo completo do app.
- Testes locais, integração sintética, Clippy, build release e smoke gráfico estão registrados em [VALIDATION.md](VALIDATION.md). Medição local de 10 s: 0,07 s CPU e 75.880 KiB RSS da GUI.
- CI para núcleo nativo/cross-target e Linux; laboratório em workflow manual. Execução remota ainda não observada.

Continuar pelo aceite completo: estados de autenticação específicos, agente de segredos/MFA/SAML, plugins/capacidades por protocolo, túneis via NetworkManager no laboratório, IKEv2, reconexão e restauração de rede. Os estados de preparação/autenticação/conexão/falha são lidos da propriedade D-Bus `VpnState`, combinados com o estado geral para não indicar conectado prematuramente. O nmcli instalado rejeitou a consulta `VPN.VPN-STATE`; não reintroduzir essa consulta ambígua. `tests/states.py` valida os estados e sinais via barramento privado; reconexão ainda não tem observação própria. O cancelamento foi testado no contrato com ativação tardia simulada e em perfil inativo; falta o cenário sob autenticação real. Cofres próprios das demais plataformas e integração completa do contrato de backend ainda não existem. Adaptadores iniciais delegados Windows/macOS estão descritos acima.

FortiClient precisa de levantamento do modo SSL-VPN/IPsec, versão de gateway e tipo de autenticação. As tarefas FORT-01/02/03 separam SSL PPP, IPsec/SAML e políticas EMS/ZTNA. Não afirmar que instalar OpenConnect resolve qualquer Fortinet. Obter ambiente de teste autorizado; não pedir para publicar credenciais ou configuração empresarial.

Prioridade de execução: homologação completa do Linux, adaptador/serviço e aplicação Windows, adaptador/mecanismo de túnel e aplicação macOS. Windows e macOS têm implementação inicial descrita acima; ainda precisam de engines, serviço/cofre, eventos e testes de túneis nativos. O núcleo segue em Rust. Confirmar requisitos de assinatura/distribuição do macOS. Android/iOS e suas bridges/extensões/aplicativos/distribuição ficam adiados.

## Publicação e privacidade

A captura local `docs/interface.png` está ignorada porque pode revelar nomes de conexões. Não publicar perfis privados, certificados, chaves, cookies, senhas, endereços de gateways ou dados reais de empresas. Usar fixtures sintéticas em testes e documentação.

As fontes foram consultadas em 12/11/2026 e devem ser revistas ao implementar cada backend. Prioridades podem mudar com a demanda; não foram prometidos prazo ou custo. O backlog registra progresso parcial; nenhum protocolo adicional foi homologado.
