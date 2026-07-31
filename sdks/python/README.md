# Greplog Python SDK

Python SDK for Greplog — automatic log, error, and HTTP capture with fail-open guarantees.

```python
import greplog
greplog.init()
greplog.info("Server started", details={"port": "4000"})
```

See [examples/basic.py](examples/basic.py) for a full working example showing configuration and manual logging.

See `docs/architecture/sdk-design.md` for the shared contract across SDKs.

