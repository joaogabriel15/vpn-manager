# Capturas da interface

Capturas reais do aplicativo GTK no Linux, com **dados fictícios** fornecidos por `tests/fixtures/preview_nmcli.py`. Nomes, usuário, endereços, CA e estado conectado são demonstrações. Nenhuma conexão foi estabelecida, nenhum perfil real foi consultado e nenhuma senha foi obtida do cofre.

| Arquivo | Tela |
| --- | --- |
| [main.png](main.png) | Lista de perfis |
| [settings.png](settings.png) | Configurações gerais |
| [network.png](network.png) | DNS e rotas IPv4 |
| [advanced.png](advanced.png) | IPv6 e opções do protocolo |
| [login.png](login.png) | Login OpenVPN com cofre opcional, sem senha preenchida |
| [compact.png](compact.png) | Editor em largura reduzida |

Para reproduzir, em uma sessão gráfica Linux com GTK, Rust, Python/PyGObject (`python-gobject` ou `python3-gi`) e `dbus-daemon`:

```sh
cargo build --release --locked
python3 tests/screenshots.py
```

O script executa uma instância separada do app em modo de teste, injeta um substituto de nmcli apenas no PATH desse processo, inicia um barramento D-Bus privado com um serviço fictício para o estado da VPN e grava somente as janelas do aplicativo. O processo usa esse barramento como system bus; o NetworkManager real não é consultado. O serviço e o barramento temporários são encerrados ao terminar. O substituto bloqueia operações que não sejam consultas de demonstração; não encaminha comandos ao NetworkManager. Os controles de cofre não são acionados. A senha sintética usada no teste de cancelamento é preenchida somente depois da captura do login.

As imagens são publicáveis no repositório. Capturas manuais de perfis reais devem continuar fora do controle de versão.
