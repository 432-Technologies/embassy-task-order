Switching lines src/main.rs:77 and src/main.rs:78 does not yield the same result !

```rust
unwrap!(spawner.spawn(sensor_reading(stts22h)));
unwrap!(spawner.spawn(pipe_data_to_usb(p.USB, p.PA12, p.PA11)));
```

When `sensor_reading` is called first, temperature readings are sensible (~20℃).
When `pipe_data` is called first, temperature readings does not make any sense, and it depends on the compilation :

-   Sometimes ~ -300℃
-   Sometimes ~ 0℃
-   Sometimes ~ -100℃
