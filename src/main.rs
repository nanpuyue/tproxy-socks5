use std::error::Error;
use std::sync::Arc;

use clap::{App, Arg};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::{TcpListener, TcpStream};
use tokio_socks::tcp::Socks5Stream;

async fn link_stream<A: AsyncRead + AsyncWrite, B: AsyncRead + AsyncWrite>(
    a: A,
    b: B,
) -> std::io::Result<()> {
    let (ar, aw) = &mut tokio::io::split(a);
    let (br, bw) = &mut tokio::io::split(b);

    let r = tokio::select! {
        r1 = tokio::io::copy(ar, bw) => {
            r1
        },
        r2 = tokio::io::copy(br, aw) => {
            r2
        }
    };

    r.map(drop)
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn Error>> {
    let matches = App::new("tproxy-socks5")
        .version("0.1.1")
        .author("南浦月 <nanpuyue@gmail.com>")
        .arg(
            Arg::with_name("local")
                .short('l')
                .value_name("ADDR:PORT")
                .help("local listen port")
                .takes_value(true)
                .required(true),
        )
        .arg(
            Arg::with_name("proxy")
                .short('x')
                .value_name("ADDR:PORT")
                .help("socks5 proxy server")
                .takes_value(true)
                .required(false),
        )
        .arg(
            Arg::with_name("remote")
                .short('r')
                .value_name("ADDR:PORT")
                .help("remote target port")
                .takes_value(true)
                .required(true),
        )
        .get_matches();

    let local = matches.value_of("local").unwrap();
    let proxy = matches.value_of("proxy").map(|x| Arc::new(x.to_owned()));
    let remote = Arc::new(matches.value_of("remote").unwrap().to_owned());

    let listener = TcpListener::bind(local).await?;
    loop {
        let (tcpstream, addr) = listener.accept().await?;
        let proxy = proxy.clone();
        let remote = remote.clone();

        tokio::spawn(async move {
            match proxy {
                Some(x) => {
                    let socks5stream = Socks5Stream::connect(x.as_str(), remote.as_str())
                        .await
                        .map_err(drop)?;
                    println!("{addr} connected");

                    link_stream(tcpstream, socks5stream).await.map_err(drop)?;
                }
                None => {
                    let upstream = TcpStream::connect(remote.as_str()).await.map_err(drop)?;
                    println!("{addr} connected");
                    link_stream(tcpstream, upstream).await.map_err(drop)?;
                }
            }

            println!("{addr} disconnected");
            Ok::<_, ()>(())
        });
    }
}
