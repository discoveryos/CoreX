use std::{env, path::PathBuf};

fn main() {
    let lwip_dir = PathBuf::from(
        env::var("LWIP_DIR").expect("LWIP_DIR must be set")
    );

    let mut core = vec![];
    let mut apps = vec![];
    let mut mbedtls = vec![];

    // ===== CORE =====
    core.extend(core_files(&lwip_dir));
    core.extend(core4_files(&lwip_dir));
    core.extend(core6_files(&lwip_dir));
    core.extend(api_files(&lwip_dir));
    core.extend(netif_files(&lwip_dir));
    core.extend(sixlowpan_files(&lwip_dir));
    core.extend(ppp_files(&lwip_dir));

    // ===== APPS =====
    apps.extend(snmp_files(&lwip_dir));
    apps.extend(http_files(&lwip_dir));
    apps.extend(makefsdata_files(&lwip_dir));
    apps.extend(iperf_files(&lwip_dir));
    apps.extend(smtp_files(&lwip_dir));
    apps.extend(sntp_files(&lwip_dir));
    apps.extend(mdns_files(&lwip_dir));
    apps.extend(netbios_files(&lwip_dir));
    apps.extend(tftp_files(&lwip_dir));
    apps.extend(mqtt_files(&lwip_dir));

    // ===== MBEDTLS =====
    mbedtls.extend(mbedtls_files(&lwip_dir));

    // ===== BUILD =====
    build_lib("lwipcore", &lwip_dir, core);
    build_lib("lwipallapps", &lwip_dir, apps);
    build_lib("lwipmbedtls", &lwip_dir, mbedtls);

    println!("cargo:rerun-if-env-changed=LWIP_DIR");
}

fn build_lib(name: &str, lwip: &PathBuf, files: Vec<PathBuf>) {
    let mut b = cc::Build::new();
    b.files(files)
        .include(lwip.join("src/include"))
        .include("include") // for your port headers
        .flag_if_supported("-Wno-unused-parameter")
        .flag_if_supported("-Wno-sign-compare")
        .compile(name);
}

// -------------------- FILE GROUPS --------------------

fn core_files(d: &PathBuf) -> Vec<PathBuf> {
    vec![
        d.join("src/core/init.c"),
        d.join("src/core/def.c"),
        d.join("src/core/dns.c"),
        d.join("src/core/inet_chksum.c"),
        d.join("src/core/ip.c"),
        d.join("src/core/mem.c"),
        d.join("src/core/memp.c"),
        d.join("src/core/netif.c"),
        d.join("src/core/pbuf.c"),
        d.join("src/core/raw.c"),
        d.join("src/core/stats.c"),
        d.join("src/core/sys.c"),
        d.join("src/core/altcp.c"),
        d.join("src/core/altcp_alloc.c"),
        d.join("src/core/altcp_tcp.c"),
        d.join("src/core/tcp.c"),
        d.join("src/core/tcp_in.c"),
        d.join("src/core/tcp_out.c"),
        d.join("src/core/timeouts.c"),
        d.join("src/core/udp.c"),
    ]
}

fn core4_files(d: &PathBuf) -> Vec<PathBuf> {
    vec![
        d.join("src/core/ipv4/acd.c"),
        d.join("src/core/ipv4/autoip.c"),
        d.join("src/core/ipv4/dhcp.c"),
        d.join("src/core/ipv4/etharp.c"),
        d.join("src/core/ipv4/icmp.c"),
        d.join("src/core/ipv4/igmp.c"),
        d.join("src/core/ipv4/ip4_frag.c"),
        d.join("src/core/ipv4/ip4.c"),
        d.join("src/core/ipv4/ip4_addr.c"),
    ]
}

fn core6_files(d: &PathBuf) -> Vec<PathBuf> {
    vec![
        d.join("src/core/ipv6/dhcp6.c"),
        d.join("src/core/ipv6/ethip6.c"),
        d.join("src/core/ipv6/icmp6.c"),
        d.join("src/core/ipv6/inet6.c"),
        d.join("src/core/ipv6/ip6.c"),
        d.join("src/core/ipv6/ip6_addr.c"),
        d.join("src/core/ipv6/ip6_frag.c"),
        d.join("src/core/ipv6/mld6.c"),
        d.join("src/core/ipv6/nd6.c"),
    ]
}

fn api_files(d: &PathBuf) -> Vec<PathBuf> {
    vec![
        d.join("src/api/api_lib.c"),
        d.join("src/api/api_msg.c"),
        d.join("src/api/err.c"),
        d.join("src/api/if_api.c"),
        d.join("src/api/netbuf.c"),
        d.join("src/api/netdb.c"),
        d.join("src/api/netifapi.c"),
        d.join("src/api/sockets.c"),
        d.join("src/api/tcpip.c"),
    ]
}

fn netif_files(d: &PathBuf) -> Vec<PathBuf> {
    let mut v = vec![
        d.join("src/netif/ethernet.c"),
        d.join("src/netif/bridgeif.c"),
        d.join("src/netif/bridgeif_fdb.c"),
    ];
    // optionally include SLIP
    if env::var("LWIP_EXCLUDE_SLIPIF").is_err() {
        v.push(d.join("src/netif/slipif.c"));
    }
    v
}

fn sixlowpan_files(d: &PathBuf) -> Vec<PathBuf> {
    vec![
        d.join("src/netif/lowpan6_common.c"),
        d.join("src/netif/lowpan6.c"),
        d.join("src/netif/lowpan6_ble.c"),
        d.join("src/netif/zepif.c"),
    ]
}

fn ppp_files(d: &PathBuf) -> Vec<PathBuf> {
    vec![
        d.join("src/netif/ppp/auth.c"),
        d.join("src/netif/ppp/ccp.c"),
        d.join("src/netif/ppp/chap-md5.c"),
        d.join("src/netif/ppp/chap_ms.c"),
        d.join("src/netif/ppp/chap-new.c"),
        d.join("src/netif/ppp/demand.c"),
        d.join("src/netif/ppp/eap.c"),
        d.join("src/netif/ppp/ecp.c"),
        d.join("src/netif/ppp/eui64.c"),
        d.join("src/netif/ppp/fsm.c"),
        d.join("src/netif/ppp/ipcp.c"),
        d.join("src/netif/ppp/ipv6cp.c"),
        d.join("src/netif/ppp/lcp.c"),
        d.join("src/netif/ppp/magic.c"),
        d.join("src/netif/ppp/mppe.c"),
        d.join("src/netif/ppp/multilink.c"),
        d.join("src/netif/ppp/ppp.c"),
        d.join("src/netif/ppp/pppapi.c"),
        d.join("src/netif/ppp/pppcrypt.c"),
        d.join("src/netif/ppp/pppoe.c"),
        d.join("src/netif/ppp/pppol2tp.c"),
        d.join("src/netif/ppp/pppos.c"),
        d.join("src/netif/ppp/upap.c"),
        d.join("src/netif/ppp/utils.c"),
        d.join("src/netif/ppp/vj.c"),
        d.join("src/netif/ppp/polarssl/arc4.c"),
        d.join("src/netif/ppp/polarssl/des.c"),
        d.join("src/netif/ppp/polarssl/md4.c"),
        d.join("src/netif/ppp/polarssl/md5.c"),
        d.join("src/netif/ppp/polarssl/sha1.c"),
    ]
}

// -------------------- APP FILES --------------------

fn snmp_files(d: &PathBuf) -> Vec<PathBuf> {
    vec![
        d.join("src/apps/snmp/snmp_asn1.c"),
        d.join("src/apps/snmp/snmp_core.c"),
        d.join("src/apps/snmp/snmp_mib2.c"),
        d.join("src/apps/snmp/snmp_mib2_icmp.c"),
        d.join("src/apps/snmp/snmp_mib2_interfaces.c"),
        d.join("src/apps/snmp/snmp_mib2_ip.c"),
        d.join("src/apps/snmp/snmp_mib2_snmp.c"),
        d.join("src/apps/snmp/snmp_mib2_system.c"),
        d.join("src/apps/snmp/snmp_mib2_tcp.c"),
        d.join("src/apps/snmp/snmp_mib2_udp.c"),
        d.join("src/apps/snmp/snmp_snmpv2_framework.c"),
        d.join("src/apps/snmp/snmp_snmpv2_usm.c"),
        d.join("src/apps/snmp/snmp_msg.c"),
        d.join("src/apps/snmp/snmpv3.c"),
        d.join("src/apps/snmp/snmp_netconn.c"),
        d.join("src/apps/snmp/snmp_pbuf_stream.c"),
        d.join("src/apps/snmp/snmp_raw.c"),
        d.join("src/apps/snmp/snmp_scalar.c"),
        d.join("src/apps/snmp/snmp_table.c"),
        d.join("src/apps/snmp/snmp_threadsync.c"),
        d.join("src/apps/snmp/snmp_traps.c"),
    ]
}

fn http_files(d: &PathBuf) -> Vec<PathBuf> {
    vec![
        d.join("src/apps/http/altcp_proxyconnect.c"),
        d.join("src/apps/http/fs.c"),
        d.join("src/apps/http/http_client.c"),
        d.join("src/apps/http/httpd.c"),
    ]
}

fn makefsdata_files(d: &PathBuf) -> Vec<PathBuf> {
    vec![d.join("src/apps/http/makefsdata/makefsdata.c")]
}

fn iperf_files(d: &PathBuf) -> Vec<PathBuf> {
    vec![d.join("src/apps/lwiperf/lwiperf.c")]
}

fn smtp_files(d: &PathBuf) -> Vec<PathBuf> {
    vec![d.join("src/apps/smtp/smtp.c")]
}

fn sntp_files(d: &PathBuf) -> Vec<PathBuf> {
    vec![d.join("src/apps/sntp/sntp.c")]
}

fn mdns_files(d: &PathBuf) -> Vec<PathBuf> {
    vec![
        d.join("src/apps/mdns/mdns.c"),
        d.join("src/apps/mdns/mdns_out.c"),
        d.join("src/apps/mdns/mdns_domain.c"),
    ]
}

fn netbios_files(d: &PathBuf) -> Vec<PathBuf> {
    vec![d.join("src/apps/netbiosns/netbiosns.c")]
}

fn tftp_files(d: &PathBuf) -> Vec<PathBuf> {
    vec![d.join("src/apps/tftp/tftp.c")]
}

fn mqtt_files(d: &PathBuf) -> Vec<PathBuf> {
    vec![d.join("src/apps/mqtt/mqtt.c")]
}

fn mbedtls_files(d: &PathBuf) -> Vec<PathBuf> {
    vec![
        d.join("src/apps/altcp_tls/altcp_tls_mbedtls.c"),
        d.join("src/apps/altcp_tls/altcp_tls_mbedtls_mem.c"),
        d.join("src/apps/snmp/snmpv3_mbedtls.c"),
    ]
}
