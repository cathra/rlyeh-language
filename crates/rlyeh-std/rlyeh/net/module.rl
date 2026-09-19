// net/module.rl：网络模块根——只做「子模块声明 + 暴露内容导出」（2026-09-19 重整）。
//
// 组成（拆分前 socket 自由函数内联于本文件，现下沉 net/socket.rl）：
//   net/byteorder.rl（字节打包：htons/sockaddr_in4_with_layout/sockaddr_in4/int_buf4/
//                     octets_to_string/parse_sockaddr）
//   net/addr.rl    （Ipv4Octets/ipv4_octets/SocketAddr/Shutdown）
//   net/tcp.rl     （TcpStream/TcpListener）
//   net/udp.rl     （UdpSocket/UdpPacket，Y7）
//   net/http.rl    （ParsedUrl/parse_url/find_header_end/parse_content_length/
//                     read_response/parse_status/Response/HttpClient）
//   net/socket.rl  （socketpair_stream/fd_at/send_all/recv_some/tcp_connect/hostname）
//
// 声明顺序注意：签名类型在收集期解析（单遍），被引用的模块须先声明。
// byteorder::parse_sockaddr 返回 net::addr::SocketAddr → addr 先于 byteorder。
// 注意：sockaddr_in4 按 macOS 布局（offset 0 = sin_len）；Linux 无 sin_len
// （offset 0-1 = sin_family），移植时需调整（条件编译待 cfg 支持）。
// 底层 extern（socketpair/socket/connect/close/r#send/r#recv/
// __rlyeh_target_os/gethostname）声明于 externs 单元。
module addr;
module byteorder;
module tcp;
module udp;
module http;
module socket;

// socket 自由函数重导出（保持 `net::socketpair_stream` 等原全名可见）。
pub import socket::socketpair_stream;
pub import socket::fd_at;
pub import socket::send_all;
pub import socket::recv_some;
pub import socket::tcp_connect;
pub import socket::hostname;
