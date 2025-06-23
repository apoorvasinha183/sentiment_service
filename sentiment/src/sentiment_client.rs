use std::{
    collections::HashMap,
    net::UdpSocket,
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

use eframe::{egui, App, CreationContext, NativeOptions, run_native};

struct MyApp {
    history: HashMap<String, Vec<[f64; 2]>>,
    visible: HashMap<String, bool>,
    rx: mpsc::Receiver<(String, f64)>,
    start: Instant,
}

impl MyApp {
    // Note: eframe will call this at startup.
    pub fn new(_cc: &CreationContext<'_>) -> Self {
        // Define your tickers and UDP ports here
        let stocks = vec![
            ("AAPL".to_string(), 3001),
            ("GOOGL".to_string(), 4001),
            ("PLTR".to_string(), 5001),
        ];

        let (tx, rx) = mpsc::channel();

        // Spawn one blocking‐UDP listener per ticker
        for (ticker, port) in stocks.clone() {
            let tx = tx.clone();
            thread::spawn(move || {
                let sock = UdpSocket::bind(("127.0.0.1", port))
                    .expect("could not bind UDP socket");
                sock.set_read_timeout(Some(Duration::from_secs(1))).unwrap();
                let mut buf = [0u8; 1024];
                while let Ok(n) = sock.recv(&mut buf) {
                    if let Ok(s) = std::str::from_utf8(&buf[..n]) {
                        if let Ok(val) = s.trim().parse::<f64>() {
                            let _ = tx.send((ticker.clone(), val));
                        }
                    }
                }
            });
        }

        // Prepare history & visibility maps
        let history = stocks.iter()
                            .map(|(t, _)| (t.clone(), Vec::new()))
                            .collect();
        let visible = stocks.iter()
                            .map(|(t, _)| (t.clone(), true))
                            .collect();

        Self {
            history,
            visible,
            rx,
            start: Instant::now(),
        }
    }
}

impl App for MyApp {
    // We no longer implement `fn name`; window title is set in `run_native`.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // 1️⃣ Ingest any new UDP samples
        while let Ok((ticker, val)) = self.rx.try_recv() {
            let t = self.start.elapsed().as_secs_f64();
            if let Some(hist) = self.history.get_mut(&ticker) {
                hist.push([t, val]);
                // Trim to last 1,000 points
                if hist.len() > 1_000 {
                    hist.drain(0..hist.len() - 1_000);
                }
            }
        }

        // 2️⃣ Side panel: ticker checkboxes
        egui::SidePanel::left("side_panel")
            .resizable(true)
            .show(ctx, |ui| {
                ui.heading("Tickers");
                ui.separator();
                for (ticker, vis) in &mut self.visible {
                    ui.checkbox(vis, ticker);
                }
            });

        // 3️⃣ Central panel: live sentiment plot
        egui::CentralPanel::default().show(ctx, |ui| {
            let plot = egui::plot::Plot::new("sentiment_plot")
                .legend(egui::plot::Legend::default())
                .view_aspect(2.0);

            plot.show(ui, |plot_ui| {
                for (ticker, hist) in &self.history {
                    if *self.visible.get(ticker).unwrap_or(&false) && !hist.is_empty() {
                        let line = egui::plot::Line::new(
                            egui::plot::PlotPoints::from(hist.clone())
                        )
                        .name(ticker.clone());
                        plot_ui.line(line);
                    }
                }
            });
        });

        // 4️⃣ Keep the UI painting for real‐time updates
        ctx.request_repaint_after(Duration::from_millis(100));
    }
}

fn main() {
    let native_options = NativeOptions::default();
    run_native(
        "Real-time Stock Sentiment Monitor", // window title
        native_options,
        Box::new(|cc| Box::new(MyApp::new(cc))),
    );
}
