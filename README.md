Tokiobeam-channel
-----------------
This project aims to combine tokio (>=0.2.0) with crossbeam (0.7.3) to provide a faster channel.  
current performance statistics looks like this:  

### Transfering 80,000 i32 numbers, calling after(200ns) per message:
```
two recv threads:   21787ns/iter, total  1743007247ns
single recv thread: 48083ns/iter, total  3846659745ns
```

### Transfering 200,000 i32 numbers, using unbounded channel
```
tokiobeam-channel:  81ns/iter, total 16226397ns
tokio-channel:     235ns/iter, total 47148280ns
```

### Transfering 200,000 i32 numbers, using bounded channel(1)
```
tokiobeam-channel:  340ns/iter, total    69172570ns
tokio-channel:     8641ns/iter, total  1728361297ns
```

### Transfering 200,000 i32 numbers, using bounded channel(1000)
```
tokiobeam-channel:  53ns/iter, total 10774397ns
tokio-channel:     304ns/iter, total 60842293ns
```

to run the benchmark, you need to turn on the `nocapture` to see the messages.  
This is because `test::Bencher` doesn't have good support on async/await that we need to estimate on our own.
```bash
cargo bench -- --nocapture --color=always
```

### Features:
- cloneable `Receiver`/`UnboundedReceiver`
- faster `AtomicWaker` (`AtomicSerialWaker`) that supports FIFO
- supports newest tokio and async/await calls
- supports bounded channel with zero capacity (core thread will be blocked, so make sure there are enough threads to run all Sender/Receivers on them)
- less cpu usage

Please report if you found any race condition issue.
