use async_channel;
use reqwest::Response;
use std::error::Error;
use tokio::{sync::oneshot::Sender, time::sleep, time::timeout, time::Duration};

pub type HttpPoolSender = async_channel::Sender<HttpPoolRequest>;
pub type HttpPoolReceiver = async_channel::Receiver<HttpPoolRequest>;
pub type HttpPoolResponse = Result<Response, reqwest::Error>;

pub struct HttpPoolRequest {
    url: String,
    // result: Sender<HttpPoolResponse>,
    result: Sender<String>,
}

#[derive(Debug)]
pub struct HttpPool {
    size: i32,
    timeout: i32,
    sender: HttpPoolSender,
    receiver: HttpPoolReceiver,
}

#[derive(Debug)]
pub struct HttpPoolBuilder {
    size: i32,
    timeout: i32,
}

impl HttpPoolBuilder {
    pub fn new() -> HttpPoolBuilder {
        HttpPoolBuilder {
            size: 10,
            timeout: 60,
        }
    }

    pub fn size(mut self, size: i32) -> HttpPoolBuilder {
        self.size = size;
        self
    }

    pub fn timeout(mut self, timeout: i32) -> HttpPoolBuilder {
        self.timeout = timeout;
        self
    }

    pub fn build(self) -> HttpPool {
        let (tx, rx) = async_channel::bounded::<HttpPoolRequest>(self.size as usize);
        HttpPool {
            size: self.size,
            timeout: self.timeout,
            sender: tx,
            receiver: rx,
        }
    }
}

impl HttpPool {
    pub fn builder() -> HttpPoolBuilder {
        HttpPoolBuilder::new()
    }

    pub fn start(self) -> Result<HttpPool, Box<dyn Error>> {
        for i in 0..self.size {
            let receiver = self.receiver.clone();
            tokio::spawn(async move {
                loop {
                    match receiver.recv().await {
                        Ok(request) => {
                            if request.result.is_closed() {
                                println!(
                                    "HttpPoolWorker[{i}]: Received closed request for url: {}",
                                    request.url
                                );
                            } else {
                                println!(
                                    "HttpPoolWorker[{i}]: Received a new request for url: {}",
                                    request.url
                                );
                                println!("Send request for url {}", request.url);
                                let resp = reqwest::get("https://httpbin.org/ip").await;
                                println!("Done Send request for url {}", request.url);

                                println!("Done request for url {}", request.url);
                                match request.result.send(request.url) {
                                        Ok(_) => println!("Send back request results to caller"),
                                        Err(_) => println!("Failed to send back the request to the caller, properly already run into a timeout")
                                    }
                            }
                        }
                        Err(err) => {
                            println!("Received error {err} during channel reading a new task")
                        }
                    }
                }
            });
            println!("Spawned new http task with id[{i}]");
        }
        Ok(self)
    }

    pub async fn request(&self, req_url: String, req_timeout: u64) -> Result<(), Box<dyn Error>> {
        let (os_sender, os_receiver) = tokio::sync::oneshot::channel::<String>();

        let request = HttpPoolRequest {
            result: os_sender,
            url: req_url.clone(),
        };

        self.sender
            .send(request)
            .await
            .expect("Failed to publish message to task group");

        // check if a timeout or value was returned
        match timeout(Duration::from_millis(req_timeout), os_receiver).await {
            Ok(res) => {
                println!("Request for url {req_url} finished without reaching the timeout");
                match res {
                    Ok(res) => {
                        println!("Request for url {req_url} receive message via result channel");
                        println!("{res}");
                        // match res {
                        //     Ok(res) => {
                        //         println!(
                        //             "Request for url {req_url} returned status code {}",
                        //             res.status()
                        //         );
                        //     }
                        //     Err(err) => {
                        //         println!("Request for url {req_url} failed with error {err}");
                        //     }
                        // }
                    }
                    Err(err) => {
                        println!("Request for url {req_url} failed to receive message {err}")
                    }
                }
            }
            Err(_) => {
                println!("Request for url {req_url} run into timeout");
            }
        }

        Ok(())
    }
}
