import tkinter as tk
from tkinter import ttk
import socket
import threading
import queue
import time
from datetime import datetime

import matplotlib
matplotlib.use('TkAgg')
from matplotlib.figure import Figure
from matplotlib.backends.backend_tkagg import FigureCanvasTkAgg
import matplotlib.dates as mdates


class SentimentGUI:
    def __init__(self, root):
        self.root = root
        self.root.title("Real-time Stock Sentiment Monitor")
        self.root.geometry("1000x600")
        self.root.configure(bg='#1e1e1e')

        # ─────────── your UDP ports & company info ───────────
        self.stocks = {
            'AAPL': {'port': 3001, 'company': 'Apple'},
            'GOOGL': {'port': 4001, 'company': 'Google Class A'},
            'PLTR': {'port': 5001, 'company': 'Palantir Technologies'},
            # add more tickers here if you like
        }

        # for storing (datetime, sentiment) history
        self.history = {ticker: [] for ticker in self.stocks}

        # for checkbuttons
        self.vars = {}

        # communication from listener threads → GUI thread
        self.update_queue = queue.Queue()

        # threading control
        self.running = True
        self.threads = []

        # build UI, start listeners, kick off refresh loops
        self.setup_ui()
        self.start_listeners()
        self.update_display()
        self.update_plot()

    def setup_ui(self):
        # ─── title/status bar ───
        title_frame = tk.Frame(self.root, bg='#1e1e1e')
        title_frame.pack(fill='x', pady=10)

        tk.Label(title_frame,
                 text="📊 Stock Sentiment Monitor",
                 font=('Arial', 24, 'bold'),
                 fg='#00ff88', bg='#1e1e1e').pack(side='left', padx=20)

        self.status_label = tk.Label(title_frame,
                                     text="🔄 Starting…",
                                     font=('Arial', 12),
                                     fg='#ffaa00', bg='#1e1e1e')
        self.status_label.pack(side='right', padx=20)

        # ─── main split: left = controls, right = plot ───
        main_frame = tk.Frame(self.root, bg='#1e1e1e')
        main_frame.pack(fill='both', expand=True)

        control_frame = tk.Frame(main_frame, bg='#1e1e1e')
        control_frame.pack(side='left', fill='y', padx=10, pady=10)

        plot_frame = tk.Frame(main_frame, bg='#1e1e1e')
        plot_frame.pack(side='right', fill='both', expand=True, padx=10, pady=10)

        # ─── ticker selection ───
        tk.Label(control_frame,
                 text="Select Tickers:",
                 font=('Arial', 14, 'bold'),
                 fg='#ffffff', bg='#1e1e1e').pack(anchor='w', pady=(0,10))

        for ticker in self.stocks:
            var = tk.BooleanVar(value=True)
            cb = tk.Checkbutton(control_frame,
                                text=f"{ticker}  ({self.stocks[ticker]['company']})",
                                variable=var,
                                font=('Arial', 12),
                                fg='#cccccc', bg='#1e1e1e',
                                selectcolor='#1e1e1e',
                                activebackground='#1e1e1e',
                                activeforeground='#00ff88')
            cb.pack(anchor='w')
            self.vars[ticker] = var

        # ─── matplotlib figure ───
        self.fig = Figure(figsize=(6,4), dpi=100)
        self.ax = self.fig.add_subplot(111)
        self.ax.set_title("Sentiment Over Time", color='#ffffff')
        self.ax.set_xlabel("Time", color='#ffffff')
        self.ax.set_ylabel("Sentiment", color='#ffffff')
        self.ax.tick_params(colors='#888888')
        self.ax.xaxis.set_major_formatter(mdates.DateFormatter('%H:%M:%S'))

        self.canvas = FigureCanvasTkAgg(self.fig, master=plot_frame)
        self.canvas.get_tk_widget().pack(fill='both', expand=True)

    def start_listeners(self):
        """Spawn one UDP listener thread per ticker."""
        for ticker, info in self.stocks.items():
            t = threading.Thread(
                target=self.listen_for_sentiment,
                args=(ticker, info['port']),
                daemon=True
            )
            t.start()
            self.threads.append(t)

        self.status_label.config(text="🟢 Listening for UDP…", fg='#00ff88')

    def listen_for_sentiment(self, ticker, port):
        """Receive float payload over UDP, push into queue."""
        try:
            sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
            sock.bind(('127.0.0.1', port))
            sock.settimeout(1.0)
            # indicate connected
            self.update_queue.put(('status', ticker, 'Connected'))
            while self.running:
                try:
                    data, _ = sock.recvfrom(1024)
                    val = float(data.decode().strip())
                    # timestamp it and queue it
                    self.update_queue.put(('sentiment', ticker, (datetime.now(), val)))
                except socket.timeout:
                    continue
                except Exception:
                    continue
        except Exception as e:
            self.update_queue.put(('status', ticker, f'Error: {e}'))

    def update_display(self):
        """Consume queue: update history & connection status."""
        try:
            while True:
                typ, ticker, payload = self.update_queue.get_nowait()
                if typ == 'sentiment':
                    ts, val = payload
                    # store history
                    self.history[ticker].append((ts, val))
                elif typ == 'status':
                    # if you want to show per-ticker status, you could expand UI.
                    pass
        except queue.Empty:
            pass

        # re-schedule
        self.root.after(100, self.update_display)

    def update_plot(self):
        """Redraw the lines for each selected ticker."""
        self.ax.clear()
        self.ax.set_title("Sentiment Over Time", color='#ffffff')
        self.ax.set_xlabel("Time", color='#ffffff')
        self.ax.set_ylabel("Sentiment", color='#ffffff')
        self.ax.tick_params(colors='#888888')
        self.ax.xaxis.set_major_formatter(mdates.DateFormatter('%H:%M:%S'))

        for ticker, var in self.vars.items():
            if var.get() and self.history[ticker]:
                times, vals = zip(*self.history[ticker])
                self.ax.plot_date(times, vals, '-', label=ticker)

        if any(var.get() for var in self.vars.values()):
            self.ax.legend(loc='upper left', facecolor='#2a2a2a', edgecolor='#444444')

        self.fig.autofmt_xdate()
        self.canvas.draw()

        # redraw every second
        self.root.after(1000, self.update_plot)

    def on_closing(self):
        self.running = False
        self.root.destroy()


def main():
    root = tk.Tk()
    app = SentimentGUI(root)
    root.protocol("WM_DELETE_WINDOW", app.on_closing)
    root.mainloop()


if __name__ == "__main__":
    main()
