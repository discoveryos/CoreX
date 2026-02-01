use std::env;
use std::path::PathBuf;

fn main() {
    let lwip_dir = PathBuf::from(env::var("LWIPDIR").expect("LWIPDIR not set"));

    let mut files: Vec<PathBuf> = vec![];

    // ===== CORE FILES =====
    files.extend(core_files(&lwip_dir));
    files.extend(core4_files(&lwip_dir));
    files.extend(core6_files(&lwip_dir));
    files.extend(api_files(&lwip_dir));
    files.extend(netif_files(&lwip_dir));
    files.extend(ppp_files(&lwip_dir));
    files.extend(sixlowpan_files(&lwip_dir));

    // ===== APPS =====
    files.extend(snmp_files(&lwip_dir));
    files.extend(http_files(&lwip_dir));
    files.extend(lwiperf_files(&lwip_dir));
    files.extend(smtp_files(&lwip_dir));
    files.extend(sntp_files(&lwip_dir));
    files.extend(mdns_files(&lwip_dir));
    files.extend(netbiosns_files(&lwip_dir));
    files.extend(tftp_files(&lwip_dir));
    files.extend(mqtt_files(&lwip_dir));
    files.extend(mbedtls_files(&lwip_dir));

    let mut build = cc::Build::new();
    build
        .files(files)
        .include(lwip_dir.join("include"))
        .include("include") // your port headers
        .flag("-Wno-unused-parameter")
        .flag("-Wno-sign-compare")
        .compile("lwip");

    println!("cargo:rerun-if-env-changed=LWIPDIR");
}

// ===== FILE GROUPS =====

fn core_files(d: &PathBuf) -> Vec<PathBuf> {
    vec![
        d.join("core/init.c"),
        d.join("core/def.c"),
        d.join("core/dns.c"),
        d.join("core/inet_chksum.c"),
        d.join("core/ip.c"),
        d.join("core/mem.c"),
        d.join("core/memp.c"),
        d.join("core/netif.c"),
        d.join("core/pbuf.c"),
        d.join("core/raw.c"),
        d.join("core/stats.c"),
        d.join("core/sys.c"),
        d.join("core/altcp.c"),
        d.join("core/altcp_alloc.c"),
        d.join("core/altcp_tcp.c"),
        d.join("core/tcp.c"),
        d.join("core/tcp_in.c"),
        d.join("core/tcp_out.c"),
        d.join("core/timeouts.c"),
        d.join("core/udp.c"),
    ]
}

fn core4_files(d: &PathBuf) -> Vec<PathBuf> {
    vec![
        d.join("core/ipv4/acd.c"),
        d.join("core/ipv4/autoip.c"),
        d.join("core/ipv4/dhcp.c"),
        d.join("core/ipv4/etharp.c"),
        d.join("core/ipv4/icmp.c"),
        d.join("core/ipv4/igmp.c"),
        d.join("core/ipv4/ip4_frag.c"),
        d.join("core/ipv4/ip4.c"),
        d.join("core/ipv4/ip4_addr.c"),
    ]
}

fn core6_files(d: &PathBuf) -> Vec<PathBuf> {
    vec![
        d.join("core/ipv6/dhcp6.c"),
        d.join("core/ipv6/ethip6.c"),
        d.join("core/ipv6/icmp6.c"),
        d.join("core/ipv6/inet6.c"),
        d.join("core/ipv6/ip6.c"),
        d.join("core/ipv6/ip6_addr.c"),
        d.join("core/ipv6/ip6_frag.c"),
        d.join("core/ipv6/mld6.c"),
        d.join("core/ipv6/nd6.c"),
    ]
}

fn api_files(d: &PathBuf) -> Vec<PathBuf> {
    vec![
        d.join("api/api_lib.c"),
        d.join("api/api_msg.c"),
        d.join("api/err.c"),
        d.join("api/if_api.c"),
        d.join("api/netbuf.c"),
        d.join("api/netdb.c"),
        d.join("api/netifapi.c"),
        d.join("api/sockets.c"),
        d.join("api/tcpip.c"),
    ]
}

fn netif_files(d: &PathBuf) -> Vec<PathBuf> {
    vec![
        d.join("netif/ethernet.c"),
        d.join("netif/bridgeif.c"),
        d.join("netif/bridgeif_fdb.c"),
        d.join("netif/slipif.c"),
    ]
}

fn sixlowpan_files(d: &PathBuf) -> Vec<PathBuf> {
    vec![
        d.join("netif/lowpan6_common.c"),
        d.join("netif/lowpan6.c"),
        d.join("netif/lowpan6_ble.c"),
        d.join("netif/zepif.c"),
    ]
}

fn ppp_files(d: &PathBuf) -> Vec<PathBuf> {
    vec![
        d.join("netif/ppp/auth.c"),
        d.join("netif/ppp/ccp.c"),
        d.join("netif/ppp/chap-md5.c"),
        d.join("netif/ppp/chap_ms.c"),
        d.join("netif/ppp/chap-new.c"),
        d.join("netif/ppp/demand.c"),
        d.join("netif/ppp/eap.c"),
        d.join("netif/ppp/ecp.c"),
        d.join("netif/ppp/eui64.c"),
        d.join("netif/ppp/fsm.c"),
        d.join("netif/ppp/ipcp.c"),
        d.join("netif/ppp/ipv6cp.c"),
        d.join("netif/ppp/lcp.c"),
        d.join("netif/ppp/magic.c"),
        d.join("netif/ppp/mppe.c"),
        d.join("netif/ppp/multilink.c"),
        d.join("netif/ppp/ppp.c"),
        d.join("netif/ppp/pppapi.c"),
        d.join("netif/ppp/pppcrypt.c"),
        d.join("netif/ppp/pppoe.c"),
        d.join("netif/ppp/pppol2tp.c"),
        d.join("netif/ppp/pppos.c"),
        d.join("netif/ppp/upap.c"),
        d.join("netif/ppp/utils.c"),
        d.join("netif/ppp/vj.c"),
        d.join("netif/ppp/polarssl/arc4.c"),
        d.join("netif/ppp/polarssl/des.c"),
        d.join("netif/ppp/polarssl/md4.c"),
        d.join("netif/ppp/polarssl/md5.c"),
        d.join("netif/ppp/polarssl/sha1.c"),
    ]
}

// ===== APPS =====

fn snmp_files(d: &PathBuf) -> Vec<PathBuf> {
    vec![
        d.join("apps/snmp/snmp_asn1.c"),
        d.join("apps/snmp/snmp_core.c"),
        d.join("apps/snmp/snmp_mib2.c"),
        d.join("apps/snmp/snmp_mib2_icmp.c"),
        d.join("apps/snmp/snmp_mib2_interfaces.c"),
        d.join("apps/snmp/snmp_mib2_ip.c"),
        d.join("apps/snmp/snmp_mib2_snmp.c"),
        d.join("apps/snmp/snmp_mib2_system.c"),
        d.join("apps/snmp/snmp_mib2_tcp.c"),
        d.join("apps/snmp/snmp_mib2_udp.c"),
        d.join("apps/snmp/snmp_snmpv2_framework.c"),
        d.join("apps/snmp/snmp_snmpv2_usm.c"),
        d.join("apps/snmp/snmp_msg.c"),
        d.join("apps/snmp/snmpv3.c"),
        d.join("apps/snmp/snmp_netconn.c"),
        d.join("apps/snmp/snmp_pbuf_stream.c"),
        d.join("apps/snmp/snmp_raw.c"),
        d.join("apps/snmp/snmp_scalar.c"),
        d.join("apps/snmp/snmp_table.c"),
        d.join("apps/snmp/snmp_threadsync.c"),
        d.join("apps/snmp/snmp_traps.c"),
    ]
}

fn http_files(d: &PathBuf) -> Vec<PathBuf> {
    vec![
        d.join("apps/http/altcp_proxyconnect.c"),
        d.join("apps/http/fs.c"),
        d.join("apps/http/http_client.c"),
        d.join("apps/http/httpd.c"),
    ]
}

fn lwiperf_files(d: &PathBuf) -> Vec<PathBuf> {
    vec![d.join("apps/lwiperf/lwiperf.c")]
}

fn smtp_files(d: &PathBuf) -> Vec<PathBuf> {
    vec![d.join("apps/smtp/smtp.c")]
}

fn sntp_files(d: &PathBuf) -> Vec<PathBuf> {
    vec![d.join("apps/sntp/sntp.c")]
}

fn mdns_files(d: &PathBuf) -> Vec<PathBuf> {
    vec![
        d.join("apps/mdns/mdns.c"),
        d.join("apps/mdns/mdns_out.c"),
        d.join("apps/mdns/mdns_domain.c"),
    ]
}

fn netbiosns_files(d: &PathBuf) -> Vec<PathBuf> {
    vec![d.join("apps/netbiosns/netbiosns.c")]
}

fn tftp_files(d: &PathBuf) -> Vec<PathBuf> {
    vec![d.join("apps/tftp/tftp.c")]
}

fn mqtt_files(d: &PathBuf) -> Vec<PathBuf> {
    vec![d.join("apps/mqtt/mqtt.c")]
}

fn mbedtls_files(d: &PathBuf) -> Vec<PathBuf> {
    vec![
        d.join("apps/altcp_tls/altcp_tls_mbedtls.c"),
        d.join("apps/altcp_tls/altcp_tls_mbedtls_mem.c"),
        d.join("apps/snmp/snmpv3_mbedtls.c"),
    ]
}
