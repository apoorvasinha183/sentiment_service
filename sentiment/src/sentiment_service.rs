// src/sentiment_service.rs
use rand::Rng;
use rand_distr::{Distribution, Normal};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    net::{Ipv4Addr, UdpSocket},
    sync::{Arc, RwLock},
    thread,
    time::Duration,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stock {
    pub ticker: String,
    pub id: u64,
    pub company_name: String,
    pub total_float: u64,
    pub initial_price: f64,
    pub sentiment_port: u64,
}

#[derive(Debug, Clone)]
pub struct SentimentConfig {
    pub tick_interval: Duration,
    pub mean: f64,
    pub reversion_speed: f64,
    pub volatility: f64,
}

impl Default for SentimentConfig {
    fn default() -> Self {
        Self {
            tick_interval: Duration::from_millis(100),
            mean: 0.0,
            reversion_speed: 0.5,
            volatility: 0.2,
        }
    }
}

pub struct SentimentService {
    stocks: Vec<Stock>,
    sentiments: Arc<RwLock<HashMap<u64, f64>>>,
    market_mood: Arc<RwLock<f64>>,
    config: SentimentConfig,
}

impl SentimentService {
    pub fn new(stocks: Vec<Stock>, config: Option<SentimentConfig>) -> Self {
        let mut sentiments = HashMap::new();
        for stock in &stocks {
            sentiments.insert(stock.id, 0.0);
        }

        Self {
            stocks,
            sentiments: Arc::new(RwLock::new(sentiments)),
            market_mood: Arc::new(RwLock::new(0.0)),
            config: config.unwrap_or_default(),
        }
    }

    pub fn from_csv(
        csv_path: &str,
        config: Option<SentimentConfig>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let mut reader = csv::Reader::from_path(csv_path)?;
        let mut stocks = Vec::new();

        for result in reader.deserialize() {
            let stock: Stock = result?;
            stocks.push(stock);
        }

        println!("Loaded {} stocks from {}", stocks.len(), csv_path);
        Ok(Self::new(stocks, config))
    }

    pub fn start(&self) {
        println!(
            "Starting sentiment service for {} stocks",
            self.stocks.len()
        );

        // Start the sentiment update engine
        self.start_sentiment_engine();

        // Start UDP broadcasters for each stock
        for stock in &self.stocks {
            self.start_udp_broadcaster(stock.clone());
        }
    }

    fn start_sentiment_engine(&self) {
        let sentiments = Arc::clone(&self.sentiments);
        let market_mood = Arc::clone(&self.market_mood);
        let stocks = self.stocks.clone();
        let config = self.config.clone();

        thread::spawn(move || {
            let mut rng = rand::thread_rng();
            let dt = config.tick_interval.as_secs_f64();
            // Create a normal distribution for the noise term
            let normal_dist = Normal::new(0.0, config.volatility).unwrap();
            let offset = 0.5;
            loop {
                thread::sleep(config.tick_interval);

                let mut mood = market_mood.write().unwrap();
                let reversion = config.reversion_speed * (config.mean - *mood) * dt;
                // Use the normal distribution to generate symmetrical noise
                let noise = normal_dist.sample(&mut rng) * dt.sqrt();
                *mood += reversion + noise;
                *mood = mood.clamp(-1.0, 1.0);

                if let Ok(mut sentiment_map) = sentiments.write() {
                    for stock in &stocks {
                        if let Some(current_sentiment) = sentiment_map.get_mut(&stock.id) {
                            let stock_noise = config.volatility * 0.1 * rng.gen_range(-1.0..1.0);
                            *current_sentiment = (*mood + stock_noise+offset).clamp(-1.0, 1.0);
                        }
                    }
                }
            }
        });
    }

    fn start_udp_broadcaster(&self, stock: Stock) {
        let sentiments = Arc::clone(&self.sentiments);
        const MULTICAST_ADDR: Ipv4Addr = Ipv4Addr::new(224, 0, 0, 123);

        thread::spawn(move || {
            let addr = format!("{}:{}", MULTICAST_ADDR, stock.sentiment_port);
            let socket = match UdpSocket::bind("0.0.0.0:0") {
                Ok(socket) => {
                    // Set a TTL to prevent packets from leaving the local network
                    socket.set_multicast_ttl_v4(1).expect("set_multicast_ttl_v4 failed");
                    println!(
                        "✓ {} ({}) broadcasting to multicast group {}",
                        stock.ticker, stock.company_name, addr
                    );
                    socket
                }
                Err(e) => {
                    eprintln!("✗ Failed to create UDP socket for {}: {}", stock.ticker, e);
                    return;
                }
            };

            loop {
                let sentiment = {
                    sentiments
                        .read()
                        .map(|map| map.get(&stock.id).copied().unwrap_or(0.0))
                        .unwrap_or(0.0)
                };

                let message = format!("{:.6}", sentiment);

                // Broadcast to multicast group - fire and forget
                if let Err(e) = socket.send_to(message.as_bytes(), &addr) {
                    eprintln!("Failed to broadcast {} sentiment: {}", stock.ticker, e);
                }

                thread::sleep(Duration::from_millis(5)); // 200 updates per second
            }
        });
    }

    pub fn get_sentiment(&self, stock_id: u64) -> f64 {
        self.sentiments
            .read()
            .map(|map| map.get(&stock_id).copied().unwrap_or(0.0))
            .unwrap_or(0.0)
    }
}

// CLI runner
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();

    let csv_path = args.get(1).map(|s| s.as_str()).unwrap_or("stock.csv");

    let config = SentimentConfig {
        tick_interval: Duration::from_millis(100),
        mean: 0.0,
        reversion_speed: 0.05,
        volatility: 0.5,
    };

    let service = SentimentService::from_csv(csv_path, Some(config))?;

    println!("🚀 Sentiment microservice starting...");
    service.start();

    // Keep main thread alive
    loop {
        thread::sleep(Duration::from_secs(1));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader};
    use std::net::TcpStream;
    use std::time::Duration;

    fn create_test_stocks() -> Vec<Stock> {
        vec![
            Stock {
                ticker: "AAPL".to_string(),
                id: 1,
                company_name: "Apple Inc.".to_string(),
                total_float: 15_982_000_000,
                initial_price: 195.37,
                sentiment_port: 18001,
            },
            Stock {
                ticker: "GOOGL".to_string(),
                id: 2,
                company_name: "Alphabet Inc.".to_string(),
                total_float: 15_982_000_000,
                initial_price: 2800.0,
                sentiment_port: 18002,
            },
        ]
    }

    #[test]
    fn test_service_creation() {
        let stocks = create_test_stocks();
        let service = SentimentService::new(stocks, None);

        assert_eq!(service.get_sentiment(1), 0.0);
        assert_eq!(service.get_sentiment(2), 0.0);
        assert_eq!(service.get_sentiment(999), 0.0); // Non-existent stock
    }

    #[test]
    fn test_udp_broadcast() {
        let stocks = create_test_stocks();
        let service = SentimentService::new(stocks, None);

        // Start service in background
        thread::spawn(move || {
            service.start();
        });

        // Give service time to start
        thread::sleep(Duration::from_millis(200));

        // Try to receive UDP data
        if let Ok(socket) = std::net::UdpSocket::bind("127.0.0.1:18001") {
            socket
                .set_read_timeout(Some(Duration::from_millis(500)))
                .ok();
            let mut buf = [0; 64];

            if let Ok((len, _)) = socket.recv_from(&mut buf) {
                let data = String::from_utf8_lossy(&buf[..len]);
                let sentiment: f64 = data.parse().unwrap_or(999.0);
                assert!(sentiment >= -1.0 && sentiment <= 1.0);
            }
        }
    }
}
