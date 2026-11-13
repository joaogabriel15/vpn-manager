Versão preliminar desktop do VPN Manager.

- OpenVPN no Linux: salvar a senha no cofre por padrão, com opção Usar senha somente desta vez, e reutilizar automaticamente nos próximos logins; atalho para trocar/esquecer e retorno ao formulário quando a tentativa falha.
- Linux: catálogo de protocolos/plugins, cadastro pelo editor nativo, login interativo opcional por nmcli --ask, importação PCF sem senhas e validação de hooks executáveis. A existência do plugin não homologa gateways.
- Linux x86_64: interface GTK e CLI com NetworkManager. Requer GTK >= 4.12, NetworkManager e os plugins dos protocolos usados (ex.: OpenVPN).
- Windows x86_64: interface e CLI para listar, solicitar conexão e desligar VPNs nativas cadastradas para o usuário Windows, com credenciais/certificados configurados no sistema.
- macOS 13+ Intel/Apple Silicon: aplicativo `.app` e CLI para os perfis VPN expostos pelo sistema em `scutil --nc`.
- Os estados vêm do sistema; uma solicitação aceita não é apresentada como conexão confirmada. A interface Windows/macOS tem atualização manual.

Windows/macOS são implementações iniciais, sem homologação de túneis reais. Não incluem engines OpenVPN/WireGuard, importação, bandeja, cofre próprio ou agente MFA. Use as configurações do sistema para cadastrar perfis e autenticação.

Os pacotes Windows não têm assinatura Authenticode. O app macOS tem assinatura ad hoc, sem Developer ID/notarização; políticas do sistema podem impedir sua execução. Não há promessa de paridade com a versão Linux.

Cada arquivo tem SHA-256 ao lado. Consulte o README incluído no pacote para instalação e limitações. Nenhuma atualização automática é executada.
