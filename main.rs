use clap::{Parser, Subcommand};
use colored::*;
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::Arc,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
    sync::{broadcast, Mutex},
};

/// Консольный TCP чат (клиент + сервер)
#[derive(Parser)]
#[command(author, version, about)]
struct Args {
    #[command(subcommand)]
    mode: Mode,
}

#[derive(Subcommand)]
enum Mode {
    /// Запуск сервера
    Server {
        /// Адрес для прослушивания
        #[arg(long)]
        addr: String,
    },
    /// Запуск клиента
    Client {
        /// Адрес сервера
        #[arg(long)]
        addr: String,

        /// Имя пользователя
        #[arg(long)]
        name: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    match args.mode {
        Mode::Server { addr } => run_server(&addr).await?,
        Mode::Client { addr, name } => run_client(&addr, &name).await?,
    }

    Ok(())
}

//////////////////////
/// SERVER
//////////////////////

async fn run_server(addr: &str) -> anyhow::Result<()> {
    let listener = TcpListener::bind(addr).await?;
    println!("?? Сервер запущен на {}", addr);

    let (tx, _) = broadcast::channel::<String>(100);
    let clients = Arc::new(Mutex::new(HashMap::<SocketAddr, ()>::new()));

    loop {
        let (socket, addr) = listener.accept().await?;
        println!("?? Подключился {}", addr);

        clients.lock().await.insert(addr, ());
        let tx = tx.clone();
        let mut rx = tx.subscribe();
        let clients = clients.clone();

        tokio::spawn(async move {
            let (mut reader, mut writer) = socket.into_split();
            let mut buffer = [0u8; 1024];

            loop {
                tokio::select! {
                    Ok(len) = reader.read(&mut buffer) => {
                        if len == 0 {
                            break;
                        }
                        let msg = String::from_utf8_lossy(&buffer[..len]).to_string();
                        let _ = tx.send(msg);
                    }

                    Ok(msg) = rx.recv() => {
                        let _ = writer.write_all(msg.as_bytes()).await;
                    }
                }
            }

            println!("? Отключился {}", addr);
            clients.lock().await.remove(&addr);
        });
    }
}

//////////////////////
/// CLIENT
//////////////////////

async fn run_client(addr: &str, name: &str) -> anyhow::Result<()> {
    let stream = TcpStream::connect(addr).await?;
    println!("? Подключено к серверу {}", addr);

    let (reader, mut writer) = stream.into_split();

    // Поток чтения сообщений
    let name_clone = name.to_string();
    tokio::spawn(async move {
        let mut reader = BufReader::new(reader);
        let mut line = String::new();

        loop {
            line.clear();
            if reader.read_line(&mut line).await.unwrap_or(0) == 0 {
                break;
            }

            if line.contains(&format!("@{}", name_clone)) {
                println!("{}", line.trim().yellow().bold());
            } else {
                println!("{}", line.trim());
            }
        }
    });

    // Ввод пользователя
    let stdin = BufReader::new(tokio::io::stdin());
    let mut lines = stdin.lines();

    while let Some(line) = lines.next_line().await? {
        let msg = format!("{}: {}\n", name, line);
        writer.write_all(msg.as_bytes()).await?;
    }

    Ok(())
}

