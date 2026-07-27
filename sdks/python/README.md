# Greplog Python SDK

Python SDK for Greplog — automatic log, error, and HTTP capture with fail-open guarantees.

```python
import greplog
from flask import Flask

app = Flask(__name__)
greplog.init(app=app)

@app.get("/")
def index():
    return "hello"
```

See `docs/architecture/sdk-design.md` for the shared contract across SDKs.
