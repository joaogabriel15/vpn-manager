//! Protocol names and integration limits, not a server certification list.
#[derive(Clone, Copy, Debug)]
pub struct VpnProtocol {
    pub id: &'static str,
    pub label: &'static str,
    pub description: &'static str,
}

pub const VPN_PROTOCOLS: &[VpnProtocol] = &[
    VpnProtocol {
        id: "wireguard",
        label: "WireGuard",
        description: "Nativo do NetworkManager. Importe um arquivo ou configure peers no editor do sistema.",
    },
    VpnProtocol {
        id: "openvpn",
        label: "OpenVPN",
        description: "Arquivos de provedores e servidores OpenVPN compatíveis. Certificados e desafios dependem do plugin.",
    },
    VpnProtocol {
        id: "openconnect",
        label: "OpenConnect",
        description: "Famílias AnyConnect/ocserv, GlobalProtect, Pulse/Ivanti, Juniper, F5, Array e Fortinet SSL/PPP conforme a versão do plugin/engine. Cada gateway e login exige validação própria.",
    },
    VpnProtocol {
        id: "fortisslvpn",
        label: "Fortinet SSL-VPN",
        description: "Plugin fortisslvpn/openfortivpn para SSL-VPN PPP. Não equivale a Fortinet IPsec, EMS ou ZTNA.",
    },
    VpnProtocol {
        id: "strongswan",
        label: "IKEv2 / strongSwan",
        description: "IKEv2/IPsec conforme autenticação e recursos do plugin strongSwan. Configure identidades e certificados no editor nativo.",
    },
    VpnProtocol {
        id: "libreswan",
        label: "IPsec / Libreswan",
        description: "IPsec conforme os modos oferecidos pelo plugin Libreswan. Não há homologação automática de fabricantes.",
    },
    VpnProtocol {
        id: "l2tp",
        label: "L2TP/IPsec",
        description: "Configure a proteção IPsec no editor do sistema. L2TP sem IPsec não oferece proteção equivalente.",
    },
    VpnProtocol {
        id: "sstp",
        label: "SSTP",
        description: "Requer plugin SSTP e autenticação/certificados compatíveis com o servidor.",
    },
    VpnProtocol {
        id: "vpnc",
        label: "Cisco IPsec legado / VPNC",
        description: "IKEv1 legado; importação Cisco PCF pelo plugin vpnc. Use somente quando necessário e autorizado pelo administrador da VPN.",
    },
    VpnProtocol {
        id: "pptp",
        label: "PPTP (legado inseguro)",
        description: "Protocolo legado com limitações de segurança conhecidas. Prefira uma VPN moderna; nenhuma escolha ou fallback automático para PPTP.",
    },
    VpnProtocol {
        id: "ssh",
        label: "Túnel SSH",
        description: "Integração pelo plugin SSH do NetworkManager, se instalado. O alcance do tráfego depende do perfil.",
    },
    VpnProtocol {
        id: "iodine",
        label: "Iodine (experimental)",
        description: "Túnel DNS pelo plugin Iodine. Encapsulamento DNS sozinho não oferece criptografia de VPN; valide proteção e alcance separadamente.",
    },
];

pub fn protocol(id: &str) -> Option<&'static VpnProtocol> {
    let id = id
        .strip_prefix("org.freedesktop.NetworkManager.")
        .unwrap_or(id);
    VPN_PROTOCOLS.iter().find(|p| p.id == id)
}

pub fn label(id: &str) -> &str {
    protocol(id).map_or(id, |p| p.label)
}
