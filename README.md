# Trading engine

It's dummy draft trading engine. My way of the exploring Rust.
Event-driven architecture.
Inline-style:
![alt text](https://github.com/saninstein/arbot/raw/master/doc/tengine.jpg "Design")

## Strategies
[ArbStrategy](src/core/strategies.rs:12)

Naive implementation of this https://reasonabledeviations.com/2019/03/02/currency-arbitrage-graphs/
Takes commissions into account.
#### Conclusion 
Makes some profits on the simulation (real price ticker and local execution). On the production execution only losses (probably latency/speed issue). Makes sense to try with colocation

## Roadmap
- Binance SBE instead of Fix protocol
- Experiment. Streams on busy loop with CPU-Affinity vs async
- Implement Orderbook, OHLCV connectors. 
