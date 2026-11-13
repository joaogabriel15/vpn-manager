# Cobertura VPN no Linux

Atualizado em 13/11/2026. O aplicativo controla perfis do **NetworkManager**. O catálogo reúne protocolos conhecidos e também aceita plugins adicionais pelo seu `vpn.service-type`. A disponibilidade exibida é descoberta localmente; ela não significa homologação de um gateway, fabricante ou método de login.

## Preparar o sistema

Instale o NetworkManager/nmcli, GTK >= 4.12 e o plugin/engine do protocolo pelo gerenciador de pacotes da distribuição. Para configurar campos específicos, instale `nm-connection-editor` ou as configurações de rede do KDE (`plasma-nm`). O editor pode precisar de um pacote gráfico adicional do plugin. O app não baixa engines nem altera as permissões Polkit.

No menu **Recursos VPN disponíveis**, confira engine, biblioteca de configuração e diálogo de autenticação. No KDE/GNOME, o agente do ambiente também pode fornecer o login mesmo quando não há um diálogo separado no plugin. Ausência de engine impede a conexão com uma mensagem de instalação; presença da biblioteca não garante que o plugin importe/exporte qualquer formato.

## Protocolos e caminhos disponíveis

| Família | Plugin / caminho | Cadastro e limites |
| --- | --- | --- |
| OpenVPN | `openvpn` | Formulário básico, arquivo detectado pelo conteúdo, editor nativo para certificados/opções e login do app ou agente do sistema |
| WireGuard | Nativo do NetworkManager | Importação de configuração e editor nativo para peers/chaves; exportação e QR próprios ainda pendentes |
| Cisco AnyConnect/ocserv, GlobalProtect, Pulse/Ivanti, Juniper, F5, Array, Fortinet SSL/PPP | `openconnect` | Selecione no editor nativo apenas os protocolos expostos pelo plugin/engine instalado; cada variante, gateway e autenticação precisa de teste próprio |
| Fortinet SSL-VPN PPP | `fortisslvpn` / openfortivpn | Configure servidor, realm/grupo, CA e autenticação no editor nativo conforme as opções oferecidas. Não equivale a Fortinet IPsec, EMS ou ZTNA |
| IKEv2/IPsec | `strongswan` | Identidades, certificados/EAP e campos próprios no editor nativo |
| IPsec | `libreswan` | Modos e autenticação conforme o plugin; não presumir paridade com strongSwan |
| L2TP/IPsec | `l2tp` | Configure a proteção IPsec. L2TP isolado não oferece proteção equivalente |
| SSTP | `sstp` | Configuração e autenticação pelo editor/plugin do sistema |
| Cisco IPsec IKEv1 legado | `vpnc` | Editor nativo e importação PCF. As senhas do arquivo são descartadas; configure-as no agente do sistema |
| PPTP | `pptp` | Legado inseguro, identificado explicitamente; sem seleção nem fallback automáticos |
| SSH | `ssh` | Plugin do sistema; alcance do túnel depende da configuração |
| Iodine | `iodine` | Experimental; encapsulamento DNS não equivale a criptografia de VPN |
| Outros plugins | Serviço completo do plugin instalado | Listagem, gestão e conexão delegadas ao NetworkManager; configuração específica no editor nativo e recursos próprios dependem do plugin |

A lista de famílias do OpenConnect vem da [documentação da engine](https://www.infradead.org/openconnect/protocols.html). Isso não comprova que todas as opções estejam expostas no plugin instalado, nem que todos os gateways dessas marcas aceitem o cliente. Perfis OpenVPN/WireGuard de provedores comerciais podem funcionar quando usam o formato padrão e não exigem recursos exclusivos do cliente oficial. Clientes proprietários sem integração NetworkManager não são controlados automaticamente pelo app.

## Cadastrar e autenticar

1. Use **Adicionar → Outros protocolos** e selecione um dos plugins com engine encontrada, ou WireGuard. Salve o perfil no editor do Linux. No KDE, o módulo de redes abre e você escolhe o tipo de conexão nele; com nm-connection-editor, o app solicita diretamente o protocolo selecionado.
2. O perfil aparece na lista por eventos do NetworkManager. A configuração específica também pode ser reaberta pela aba Avançado. O diálogo interno fecha antes de abrir o editor externo, para não salvar campos antigos sobre alterações feitas no sistema.
3. Use **Conectar**. Perfis OpenVPN mantêm o formulário do aplicativo; os demais usam o agente do ambiente. Sem agente compatível, use **Login interativo no terminal** no menu do perfil ou a CLI com `--ask`.

O login interativo usa `nmcli --ask`, sem colocar credenciais nos argumentos. A GUI encontra Konsole, GNOME Terminal ou xterm; outros terminais podem executar a CLI diretamente. Prompts de senha, OTP ou certificado dependem da integração do plugin. SAML/OIDC, autenticação no navegador e qualquer tipo de MFA não são garantidos. Configure certificados e a confiança no servidor conforme as orientações do administrador; o app não desativa a verificação de identidade como fallback.

A abertura do terminal/editor é uma operação delegada, não confirmação de conexão. O estado vem do NetworkManager. Para interromper um login interativo, cancele no terminal; se o perfil continuar em transição, use **Cancelar** no app. O cancelamento determinístico com token continua no fluxo de conexão controlado diretamente pelo app; processos externos têm seu próprio ciclo de vida.

## Salvar senhas e conectar sem redigitar

No login OpenVPN, o app salva a senha no cofre por padrão após conectar com sucesso. Para impedir o salvamento ou a substituição de uma senha anterior, marque **Usar senha somente desta vez (não salvar)** naquele login. Após o primeiro sucesso, o app busca essa senha no cofre Secret Service ao conectar e não abre o formulário novamente. Itens continuam separados por perfil/usuário e senhas não são enviadas como argumentos. O cofre pode pedir desbloqueio conforme as regras da sessão.

Para trocar ou esquecer a senha, abra **Senha salva e login…** no menu do perfil desconectado. Se a tentativa com a senha salva falhar, o app abre o formulário para correção, sem repetir a credencial indefinidamente. Senhas substitutas só são gravadas após sucesso e fora do modo de uso único; cancelar não grava. O modo de uso único preserva qualquer senha que já estivesse no cofre. Sem um cofre disponível, o app informa que usará a senha somente nessa conexão. **Esquecer senha** também limpa a senha transitória do formulário.

Nos demais protocolos, **Salvar credenciais no sistema…** abre o editor nativo; escolha o armazenamento no cofre do usuário conforme as opções do plugin/agente. Tokens temporários/OTP e políticas corporativas ainda podem exigir novos desafios; não são tratados como uma senha fixa pelo app.

## Importação e exportação

- Conteúdo reconhecido: OpenVPN, WireGuard, Cisco PCF com `[main]`, `Host` e `GroupName`, e VPN Manager TOML. A extensão não define o protocolo. Conteúdo desconhecido ou misto é rejeitado; configure outros protocolos pelo editor nativo.
- Scripts/hooks OpenVPN e WireGuard e includes executáveis OpenVPN são rejeitados pela política de importação. Certificados e chaves inline não são interpretados como diretivas. `script-security` 0–2 sem hooks é aceito, preservando as exportações padrão do plugin OpenVPN; nível 3 é rejeitado.
- PCF é passado ao plugin em arquivo temporário privado sem `UserPassword`, `enc_UserPassword`, `GroupPwd` ou `enc_GroupPwd`. A escolha de salvar senha do PCF é desativada. Isso evita o caminho do importador vpnc que chama cisco-decrypt com o segredo codificado como argumento; o usuário deve configurar as credenciais pelo sistema.
- Importadores e exportadores continuam responsáveis pelos formatos e opções que suportam. Erros de importação do plugin não reproduzem a configuração. Exportar exige ação explícita, pode incluir credenciais/referências privadas, grava com permissão 0600 e não sobrescreve arquivos existentes. WireGuard ainda não oferece exportação neste adaptador.

## CLI Linux

```sh
vpn-manager-cli catalog
vpn-manager-cli capabilities
vpn-manager-cli list
vpn-manager-cli create-native openconnect
vpn-manager-cli create-native org.freedesktop.NetworkManager.fortisslvpn
vpn-manager-cli edit <uuid>
vpn-manager-cli validate-import <arquivo>
vpn-manager-cli import <arquivo>
vpn-manager-cli connect <uuid> --ask
vpn-manager-cli disconnect <uuid>
vpn-manager-cli export <uuid> <arquivo-novo>
```

`catalog` descreve as famílias conhecidas; `capabilities` inclui serviços adicionais descobertos. `validate-import` verifica formato e política, sem garantir que o plugin aceite todas as opções. Importar PCF não importa senhas.

## Validação disponível e trabalho restante

Os testes de integração cadastram perfis inativos de 11 serviços, incluindo um plugin sintético ausente, verificam protocolo/estado, edição de DNS e remoção, e recusam ativação do plugin ausente antes de iniciar o túnel. Também há integração OpenVPN, exportação privada sem sobrescrita e rollback. Nenhum desses testes ativa uma VPN corporativa.

O laboratório anterior validou tráfego das engines OpenVPN/WireGuard isoladas; o fluxo completo aplicativo → NetworkManager → gateway continua pendente. A sanitização PCF foi testada com fixture; o plugin vpnc não estava instalado nesta máquina e sua importação real ainda precisa de validação. Também faltam gateways autorizados, login/MFA, tráfego/DNS/IPv6, suspensão, reconexão e restauração por família. Consulte [VALIDATION.md](VALIDATION.md).

Referências: [nmcli e agentes de segredos](https://networkmanager.pages.freedesktop.org/NetworkManager/NetworkManager/nmcli.html), [opções do editor nativo](https://github.com/GNOME/network-manager-applet/blob/main/src/connection-editor/main.c) e [importador/exportador VPNC](https://github.com/NetworkManager/NetworkManager-vpnc/blob/main/properties/nm-vpnc-editor-plugin.c).
