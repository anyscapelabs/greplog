#!/usr/bin/env python3
import argparse, json, random, os, sys
from datetime import datetime, timezone, timedelta
from pathlib import Path
import pyarrow as pa
import pyarrow.parquet as pq

SERVICES = [
    ("api-gateway", 22),
    ("auth-service", 12),
    ("payment-service", 15),
    ("order-service", 14),
    ("notification-service", 10),
    ("search-service", 11),
    ("inventory-service", 8),
    ("user-service", 8),
]
LEVELS = [
    ("INFO", 52),
    ("DEBUG", 14),
    ("WARN", 13),
    ("ERROR", 9),
    ("TRACE", 5),
    ("FATAL", 2),
    ("CRITICAL", 3),
    ("UNKNOWN", 2),
]

def pick_weighted(rng, items):
    total = sum(w for _, w in items)
    n = rng.randint(0, total-1)
    for name, w in items:
        if n < w:
            return name
        n -= w
    return items[0][0]

def service_message(service, level, rng):
    # same templates as rust
    if service == "api-gateway":
        if level == "INFO":
            return rng.choice([
                "GET /api/v1/users 200 OK",
                "POST /api/v1/orders 201 Created",
                "GET /api/v1/products 200 OK - cache hit",
                "PUT /api/v1/users/profile 200 OK",
                "DELETE /api/v1/cart/item 204 No Content",
                "GET /api/v1/health 200 OK - latency 12ms",
            ])
        if level == "WARN":
            return rng.choice([
                "Rate limit near threshold for IP 192.168.1.42 - 85% quota used",
                "Upstream latency spike: auth-service p95 342ms",
                "Deprecated endpoint GET /api/v0/users called",
                "High memory usage: 78% heap utilized",
            ])
        if level == "ERROR":
            return rng.choice([
                "Request timeout on upstream payment-service - 504 Gateway Timeout",
                "Failed to proxy POST /api/v1/checkout - upstream returned 502",
                "CORS rejection for origin https://evil.example.com",
                "TLS handshake failed for downstream client",
            ])
        return rng.choice([
            "Cache miss for key user:profile:usr_7821",
            "Routing request to shard us-east-1a",
            "JWT validated for user usr_1234 - scopes: read,write",
            "Compression enabled: gzip ratio 0.34",
        ])
    if service == "auth-service":
        if level == "INFO":
            return rng.choice(["User signed in successfully","Token refreshed for user","Password reset completed","MFA challenge passed","Session created for new device"])
        if level == "WARN":
            return rng.choice(["Failed login attempt - invalid password","Brute force detection: 5 attempts in 60s","Weak password rejected for new user","Session expiring soon - refresh advised"])
        if level == "ERROR":
            return rng.choice(["Token generation failed: signing key expired","OAuth provider unreachable - github auth failed","Database connection lost during authentication","Account locked after 5 failed attempts"])
        return rng.choice(["Auth middleware processed request","LDAP sync completed for 234 users","Permission check cache invalidated"])
    if service == "payment-service":
        if level == "INFO":
            return rng.choice(["Payment succeeded for order","Refund processed successfully","Webhook delivered to merchant","Settlement batch completed","Payment intent confirmed"])
        if level == "WARN":
            return rng.choice(["Payment retry scheduled - attempt 2/3","Card expiry warning: card ending 4242 expires next month","Dispute evidence required for charge","Currency conversion rounding applied"])
        if level == "ERROR":
            return rng.choice(["Payment failed: card_declined - insufficient funds","Payment failed: processor timeout after 30s","Fraud detection blocked transaction - risk_score 0.92","Refund failed: charge already refunded","Stripe API error 429 - rate limited"])
        return rng.choice(["Payment ledger reconciled for day","Idempotency key cache hit - duplicate suppressed","3DS challenge initiated for SCA"])
    if service == "order-service":
        if level == "INFO":
            return rng.choice(["Order created successfully","Order shipped - tracking generated","Order delivered confirmed by carrier","Cart checkout completed","Order status updated to processing"])
        if level == "WARN":
            return rng.choice(["Inventory low for SKU-7821 - only 3 left","Order delayed - warehouse backlog 2h","Partial fulfillment: 2/3 items shipped","Duplicate order detection triggered"])
        if level == "ERROR":
            return rng.choice(["Order creation failed: inventory check failed","Fulfillment failed: warehouse API 503","Payment capture mismatch for order","Order validation failed: invalid SKU"])
        return rng.choice(["Order lifecycle event emitted to queue","Prorate calculation for discount applied","Inventory reservation extended"])
    if service == "notification-service":
        if level == "INFO":
            return rng.choice(["Email sent: order confirmation","Push notification delivered successfully","SMS OTP sent to user","Webhook dispatched to partner"])
        if level == "WARN":
            return rng.choice(["Email bounced - mailbox full","Push token expired - need refresh","Rate limit for SMS: 60/hour exceeded","Template rendering slow - 180ms"])
        if level == "ERROR":
            return rng.choice(["Failed to send email: SES throttled","Push delivery failed: FCM 503 unavailable","SMS provider timeout - Twilio 504","Notification queue depth critical: 12k pending"])
        return rng.choice(["Notification preference checked for user","Batch digest compiled for 1.2k users","Unsubscribe handled for campaign"])
    if service == "search-service":
        if level == "INFO":
            return rng.choice(["Search query executed: 'wireless headphones' - 234 results","Indexing completed for 1.2k products","Autocomplete suggestions served in 12ms","Relevance scoring updated for category"])
        if level == "WARN":
            return rng.choice(["Slow query: search took 420ms - threshold 300ms","Elasticsearch shard relocation in progress","Low relevance score for query - fallback to TF-IDF","Cache eviction: 40% of query cache cleared"])
        if level == "ERROR":
            return rng.choice(["Search backend timeout after 2s","Index corrupted for shard 3 - rebuilding","Query parsing failed: syntax error near 'AND'","Elasticsearch cluster health red - 2 nodes down"])
        return rng.choice(["Search analytics event recorded","Synonym expansion applied for query","Faceted search filtered by 3 attributes"])
    if service == "inventory-service":
        if level == "INFO":
            return rng.choice(["Stock updated for SKU-4412: +50 units","Inventory sync completed with warehouse EU-1","Reservation confirmed for 3 items","Cycle count reconciled - variance 0.2%"])
        if level == "WARN":
            return rng.choice(["Deadstock alert: SKU-9921 no movement 90 days","Reorder point reached for SKU-1234","Warehouse sync lag 4.2s - above threshold","Negative inventory prevented for SKU-5511"])
        if level == "ERROR":
            return rng.choice(["Inventory update failed: optimistic lock conflict","Warehouse API unreachable - sync failed","Stock deduction overflow for bundle order","Database deadlock on inventory transaction"])
        return rng.choice(["Inventory forecast recalculated for Q3","ABC analysis updated for 12k SKUs","Batch stock import processed: 234 rows"])
    if service == "user-service":
        if level == "INFO":
            return rng.choice(["User profile updated successfully","New user registered: onboarding started","Avatar uploaded and processed","User preferences saved","Account verified via email"])
        if level == "WARN":
            return rng.choice(["Profile completeness low: 45% - prompt user","Stale session cleaned for inactive user","Duplicate email signup attempt blocked","GDPR deletion request queued"])
        if level == "ERROR":
            return rng.choice(["Failed to fetch user: not found","Profile update conflict - version mismatch","Avatar processing failed: invalid image format","User creation failed: email already exists"])
        return rng.choice(["User segmentation recalculated for cohort","Feature flag evaluated for user","Audit log entry created for compliance"])
    return f"{service} handled {level} event"

def raw_body_for(service, level, message, rng):
    if rng.randint(0,99) < 18:
        return None
    user_id = f"usr_{rng.randint(1000,9999)}"
    order_id = rng.randint(10000,99999)
    amount = round(rng.randint(500,25000)/100,2)
    latency = rng.randint(5,980)
    status = rng.choice([200,201,204,400,401,403,404,429,500,502,503,504])
    trace_hex = "".join(f"{rng.randint(0,15):x}" for _ in range(8))
    ip = f"192.168.{rng.randint(1,254)}.{rng.randint(1,254)}"
    if service == "api-gateway":
        body = {"method": rng.choice(["GET","POST","PUT","DELETE","PATCH"]), "path": rng.choice(["/api/v1/users","/api/v1/orders","/api/v1/products","/api/v1/cart","/api/v1/auth/login"]), "status": status, "latency_ms": latency, "ip": ip, "user_id": user_id, "trace": trace_hex, "bytes": rng.randint(200,15000), "user_agent": "Mozilla/5.0"}
    elif service == "auth-service":
        body = {"user_id": user_id, "ip": ip, "action": rng.choice(["login","logout","refresh","mfa_verify","password_reset"]), "success": level!="ERROR", "latency_ms": latency, "provider": rng.choice(["local","google","github","okta"]), "reason": message if level=="ERROR" else "ok"}
    elif service == "payment-service":
        body = {"order_id": order_id, "user_id": user_id, "amount": amount, "currency": rng.choice(["USD","EUR","GBP"]), "payment_method": rng.choice(["card","paypal","apple_pay","google_pay"]), "status": "failed" if level=="ERROR" else "retry" if level=="WARN" else "succeeded", "processor": rng.choice(["stripe","braintree","adyen"]), "risk_score": round(rng.random(),3), "retry_count": rng.randint(1,3) if level=="WARN" else 0}
    elif service == "order-service":
        body = {"order_id": order_id, "user_id": user_id, "items": rng.randint(1,5), "total": amount, "warehouse": rng.choice(["EU-1","US-WEST","US-EAST","APAC-1"]), "shipping_method": rng.choice(["standard","express","overnight"]), "status": rng.choice(["created","processing","shipped","delivered"])}
    elif service == "notification-service":
        body = {"channel": rng.choice(["email","push","sms","webhook"]), "recipient": user_id, "template": rng.choice(["order_confirmation","shipping_update","password_reset","promo"]), "delivery_ms": latency, "provider": rng.choice(["ses","fcm","twilio","sendgrid"]), "attempt": rng.randint(1,3)}
    elif service == "search-service":
        body = {"query": rng.choice(["wireless headphones","laptop stand","coffee beans","yoga mat","running shoes"]), "results": rng.randint(0,500), "latency_ms": latency, "index": rng.choice(["products","users","orders"]), "shard": rng.randint(1,5), "cache_hit": rng.choice([True, False])}
    elif service == "inventory-service":
        body = {"sku": f"SKU-{rng.randint(1000,9999)}", "warehouse": rng.choice(["EU-1","US-WEST","US-EAST"]), "delta": rng.randint(-50,100), "remaining": rng.randint(0,500), "reason": rng.choice(["sale","restock","return","audit"]), "latency_ms": latency}
    elif service == "user-service":
        body = {"user_id": user_id, "action": rng.choice(["profile_update","register","avatar_upload","preferences_save"]), "fields_changed": rng.randint(1,3), "ip": ip, "session_id": f"sess_{trace_hex}"}
    else:
        body = {"message": message, "latency_ms": latency}
    return json.dumps(body, separators=(",",":") )

def generate_timestamp_us(rng, now_us):
    r = rng.random()
    if r < 0.45:
        offset = rng.randint(0, 86400-1)
    elif r < 0.70:
        offset = rng.randint(86400, 172800-1)
    elif r < 0.85:
        offset = rng.randint(172800, 259200-1)
    elif r < 0.93:
        offset = rng.randint(259200, 432000-1)
    else:
        offset = rng.randint(432000, 604800-1)
    jitter = rng.randint(0, 1_000_000-1)
    ts = now_us - offset*1_000_000 - jitter
    if rng.randint(0,99) < 5:
        ts = now_us - rng.randint(0, 300)*1_000_000 - rng.randint(0, 1_000_000-1)
    return ts

def main():
    parser = argparse.ArgumentParser(description="Greplog 100k seeder - direct Parquet")
    parser.add_argument("--count", type=int, default=100000, help="number of logs")
    parser.add_argument("--clean", action="store_true", help="remove existing parquet data")
    parser.add_argument("--data-dir", default="/home/brnx/Desktop/greplog-workspace/greplog/data/logs")
    parser.add_argument("--seed", type=int, default=42)
    args = parser.parse_args()
    rng = random.Random(args.seed)
    data_dir = Path(args.data_dir)
    count = args.count

    print(f"Greplog 100k seeder (Python)")
    print(f"  count   : {count}")
    print(f"  data_dir: {data_dir}")
    print(f"  seed    : {args.seed}")
    print(f"  clean   : {args.clean}")

    if args.clean and data_dir.exists():
        print(f"Cleaning {data_dir} ...")
        for child in list(data_dir.iterdir()):
            if child.is_dir():
                import shutil; shutil.rmtree(child)
                print(f"  removed {child}")
            else:
                child.unlink()
                print(f"  removed {child}")
        print("Cleaned.")
    data_dir.mkdir(parents=True, exist_ok=True)

    now = datetime.now(timezone.utc)
    now_us = int(now.timestamp()*1_000_000)
    print(f"Generating {count} logs with now={now.isoformat()} now_us={now_us}")

    # Collect records by partition key
    partitions = {}  # (year,month,day,service) -> list of dicts
    service_counts = {}
    level_counts = {}
    for idx in range(count):
        service = pick_weighted(rng, SERVICES)
        level = pick_weighted(rng, LEVELS)
        ts_us = generate_timestamp_us(rng, now_us)
        # convert ts_us to date for partition
        dt = datetime.fromtimestamp(ts_us/1_000_000, tz=timezone.utc)
        year, month, day = dt.year, dt.month, dt.day
        msg = service_message(service, level, rng)
        if rng.random() < 0.3:
            msg = f"{msg} | order #{rng.randint(10000,99999)} user usr_{rng.randint(1000,9999)}"
        trace_id = None
        if rng.randint(0,99) < 75:
            if rng.random() < 0.5:
                trace_id = f"trace-{rng.randint(0, 0xffffffff):08x}{rng.randint(0, 0xffffffff):08x}"
            else:
                trace_id = f"req_{now.strftime('%Y%m%d')}-{rng.randint(0,0xffff):04x}"
        raw_body = raw_body_for(service, level, msg, rng)

        key = (year, month, day, service)
        if key not in partitions:
            partitions[key] = []
        partitions[key].append((ts_us, trace_id, level, msg, raw_body))

        service_counts[service] = service_counts.get(service,0)+1
        level_counts[level] = level_counts.get(level,0)+1
        if (idx+1) % 20000 == 0:
            print(f"  generated {idx+1}/{count}")

    print(f"\nPartitions: {len(partitions)} (service x day combos)")
    total_bytes = 0
    total_rows = 0
    for (year, month, day, service), rows in sorted(partitions.items()):
        # sort rows by timestamp ascending for nice parquet
        rows.sort(key=lambda x: x[0])
        ts_list = [r[0] for r in rows]
        trace_list = [r[1] for r in rows]
        level_list = [r[2] for r in rows]
        msg_list = [r[3] for r in rows]
        raw_list = [r[4] for r in rows]

        # Build Arrow table without service column (service is partition)
        # timestamp_us as timestamp[us]
        ts_array = pa.array(ts_list, type=pa.timestamp('us'))
        trace_array = pa.array(trace_list, type=pa.string())
        level_array = pa.array(level_list, type=pa.string())  # plain string, engine supports dictionary via cast
        msg_array = pa.array(msg_list, type=pa.string())
        raw_array = pa.array(raw_list, type=pa.string())

        table = pa.table({
            "timestamp_us": ts_array,
            "trace_id": trace_array,
            "level": level_array,
            "message": msg_array,
            "raw_body": raw_array,
        })

        # Schema order must be timestamp_us, trace_id, level, message, raw_body (service omitted)
        # but we enforce that order via table names above

        dir_path = data_dir / f"year={year}" / f"month={month:02d}" / f"day={day:02d}" / f"service={service}"
        dir_path.mkdir(parents=True, exist_ok=True)
        # chunk file name like chunk_<nanos>.parquet
        nanos = int(datetime.now(timezone.utc).timestamp()*1_000_000_000) + rng.randint(0, 1_000_000)
        # ensure uniqueness per partition by adding random
        file_path = dir_path / f"chunk_{nanos}.parquet"
        part_path = dir_path / f"chunk_{nanos}.parquet.part"
        # Write
        pq.write_table(table, str(part_path), compression='SNAPPY')
        part_path.rename(file_path)
        total_bytes += file_path.stat().st_size
        total_rows += len(rows)
        # print per partition
        print(f"  {year}-{month:02d}-{day:02d} {service:<22} {len(rows):>5} rows -> {file_path.relative_to(data_dir)} ({file_path.stat().st_size/1024:.1f} KB)")

    print(f"\nDone - wrote {total_rows} rows across {len(partitions)} chunks")
    print(f"Total bytes: {total_bytes} ({total_bytes/1024/1024:.2f} MB)")

    print("\nService distribution:")
    for svc, cnt in sorted(service_counts.items(), key=lambda x: -x[1]):
        print(f"  {svc:<22} {cnt:>6} ({cnt/count*100:.1f}%)")
    print("\nLevel distribution:")
    for lvl, cnt in sorted(level_counts.items(), key=lambda x: -x[1]):
        print(f"  {lvl:<10} {cnt:>6} ({cnt/count*100:.1f}%)")

    # Verify via reading back row counts
    found = 0
    for p in data_dir.rglob("*.parquet"):
        try:
            pf = pq.ParquetFile(str(p))
            found += pf.metadata.num_rows
        except: pass
    print(f"\nVerification - total parquet rows on disk (all partitions): {found}")
    if found == count and args.clean:
        print(" ✅ Row count matches requested count!")
    elif not args.clean:
        print(f" note: {found} includes pre-existing data + {count} new rows")

if __name__ == "__main__":
    main()
