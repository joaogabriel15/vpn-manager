# Notas para retomar o desenvolvimento

Atualizado em 12/11/2026.

## Projeto e conta

- Projeto independente: `/home/pacheco/Projetos/vpn-manager`. Não pertence ao Excalibur.
- Repositório público: https://github.com/joaogabriel15/vpn-manager
- Conta GitHub a usar: **joaogabriel15**, conforme pedido explícito do proprietário.
- Antes de publicar: conferir `gh api user --jq .login` e `git remote get-url origin`; não usar outra conta autenticada.
- Git local configurado com e-mail noreply da conta pessoal.

## Pedido registrado

Pesquisar famílias de VPN e criar um backlog para uso amplo entre protocolos e sistemas operacionais. Incluir Linux, Windows, macOS, Android e iOS/iPadOS, avaliar sistemas adicionais e tratar FortiClient/Fortinet explicitamente. O pedido desta etapa é planejamento, não implementação de todos os conectores.

## Onde está o planejamento

- [Backlog priorizado](../BACKLOG.md): 61 itens, dependências, esforço relativo, critérios de aceite e marcos.
- [Inventário de VPNs](VPN-COMPATIBILITY.md): protocolos, fabricantes, Fortinet, redes mesh, provedores e pesquisas adjacentes; referências primárias.
- [Plano de plataformas](PLATFORMS.md): núcleo Rust compartilhado, adaptadores nativos, limites de cada sistema e requisitos de distribuição.

Nenhuma pesquisa deve ser apresentada como compatibilidade já entregue. Não há como garantir literalmente todos os produtos e versões; a estratégia é catálogo extensível e homologação por combinação.

## Estado entregue antes deste backlog

App Linux em Rust/GTK com NetworkManager, bandeja e instalação por usuário. Interface compacta: busca, lista de perfis, conectar e menus. Ícones do KDE corrigidos com identificação da aplicação e recursos de ícone adequados. Modais têm rodapé com margens; os botões reorganizam em largura reduzida. Fluxos OpenVPN/WireGuard existem, mas testes de interface e perfis temporários não homologam túneis reais.

Manter a interface simples e prática, sem painéis decorativos ou textos promocionais. Preservar consumo reduzido e atualização por eventos. O backlog não autoriza reescrever a UI antes de validar a arquitetura.

## Próximo ponto técnico

Começar por CORE-01/02/03/04: separar núcleo, contrato de adaptadores, estados e perfis. O backend atual usa nmcli, assume um segredo específico de VPN e ainda não resolve autenticação genérica. Depois, AUTH-01/02, laboratório QA-01 e conclusão/homologação de OpenVPN e WireGuard.

FortiClient precisa de levantamento do modo SSL-VPN/IPsec, versão de gateway e tipo de autenticação. As tarefas FORT-01/02/03 separam SSL PPP, IPsec/SAML e políticas EMS/ZTNA. Não afirmar que instalar OpenConnect resolve qualquer Fortinet. Obter ambiente de teste autorizado; não pedir para publicar credenciais ou configuração empresarial.

Windows, Apple e Android precisam de adaptadores e mecanismos próprios. O núcleo segue em Rust; APIs móveis, extensões, permissões e assinatura não são substituídas pelo backend Linux. Confirmar requisitos Apple de distribuição antes de prometer release na loja.

## Publicação e privacidade

A captura local `docs/interface.png` está ignorada porque pode revelar nomes de conexões. Não publicar perfis privados, certificados, chaves, cookies, senhas, endereços de gateways ou dados reais de empresas. Usar fixtures sintéticas em testes e documentação.

As fontes foram consultadas em 12/11/2026 e devem ser revistas ao implementar cada backend. Prioridades podem mudar com a demanda; não foram prometidos prazo ou custo. Todas as tarefas pendentes do backlog continuam A fazer.
