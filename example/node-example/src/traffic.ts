/**
 * Simulated Amazon backend traffic generator.
 *
 * Emits a steady stream of realistic e-commerce backend logs — product
 * search, cart operations, checkout, payments, orders, inventory, auth, and
 * downstream dependency calls — with the full structured field set that a
 * production Amazon-style backend would record for each event:
 *
 *   timestamp_us  — set automatically by the SDK (microseconds since epoch)
 *   trace_id      — correlation id spanning the whole request tree
 *   level         — INFO / WARN / ERROR
 *   service       — the backend module that produced the event
 *   message       — human-readable summary
 *   raw_body      — stringified JSON with every domain field (order, user,
 *                   product, pricing, latency, region, datacenter, ...)
 *
 * Run with `npm run traffic` (Node 24+ runs TypeScript natively).
 */

import greplog from 'greplog-sdk'

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/** Requests per second to emit. Override with the TRAFFIC_RPS env var. */
const RPS = Number(process.env.TRAFFIC_RPS ?? 2)

/** Which backend service the events are tagged with. */
const SERVICE_GROUP = process.env.TRAFFIC_SERVICE ?? 'amazon-backend'

/** Endpoint override; defaults to the local ingest agent. */
const ENDPOINT = process.env.GREPLOG_URL

// ---------------------------------------------------------------------------
// Weighted random helpers
// ---------------------------------------------------------------------------

function pick<T>(weighted: Array<[T, number]>): T {
  const total = weighted.reduce((sum, [, weight]) => sum + weight, 0)
  let roll = Math.random() * total
  for (const [value, weight] of weighted) {
    roll -= weight
    if (roll <= 0) return value
  }
  return weighted[weighted.length - 1][0]
}

function pickList<T>(items: readonly T[]): T {
  return items[Math.floor(Math.random() * items.length)]
}

function randomInt(min: number, max: number): number {
  return Math.floor(Math.random() * (max - min + 1)) + min
}

function money(min: number, max: number): number {
  return Math.round((Math.random() * (max - min) + min) * 100) / 100
}

/** Short random trace id, e.g. `1-abc123-xyz`. */
function newTraceId(): string {
  const hex = (len: number) =>
    Array.from({ length: len }, () =>
      '0123456789abcdef'.charAt(Math.floor(Math.random() * 16)),
    ).join('')
  return `1-${hex(8)}-${hex(8)}`
}

// ---------------------------------------------------------------------------
// Realistic lookup tables (approximate Amazon catalog / fleet)
// ---------------------------------------------------------------------------

const USERS = [
  { user_id: 'usr_8125_maria', account_type: 'prime', market: 'US' },
  { user_id: 'usr_6331_james', account_type: 'standard', market: 'US' },
  { user_id: 'usr_9902_anna', account_type: 'prime', market: 'DE' },
  { user_id: 'usr_4471_kenji', account_type: 'prime', market: 'JP' },
  { user_id: 'usr_2058_lucas', account_type: 'business', market: 'GB' },
  { user_id: 'usr_7310_sara', account_type: 'standard', market: 'IN' },
]

const ASINS = [
  { asin: 'B0D1XYZ001', title: 'Kindle Paperwhite 11th Gen', category: 'Electronics' },
  { asin: 'B0D1XYZ002', title: 'Echo Dot (5th Gen)', category: 'Electronics' },
  { asin: 'B0D1XYZ003', title: 'Organic Green Tea 100ct', category: 'Grocery' },
  { asin: 'B0D1XYZ004', title: "Men's Running Shoes 10", category: 'Apparel' },
  { asin: 'B0D1XYZ005', title: 'Stainless Steel Water Bottle', category: 'Home & Kitchen' },
  { asin: 'B0D1XYZ006', title: 'Bluetooth Mechanical Keyboard', category: 'Electronics' },
  { asin: 'B0D1XYZ007', title: 'Baby Diapers Size 3 (Pack of 96)', category: 'Baby' },
  { asin: 'B0D1XYZ008', title: 'Peanut Butter Natural 32oz', category: 'Grocery' },
  { asin: 'B0D1XYZ009', title: 'USB-C Fast Charger 65W', category: 'Electronics' },
  { asin: 'B0D1XYZ010', title: 'Cotton Bed Sheet Set Queen', category: 'Home & Kitchen' },
]

const SEARCH_QUERIES = [
  'bluetooth headphones',
  'kipling backpack',
  'coffee beans',
  'ps5 games',
  'led lights strip',
  'mattress protector',
  'air fryer',
  'yoga mat',
  'cat food',
  'usb hub',
]

const REGIONS = ['us-east-1', 'us-west-2', 'eu-west-1', 'eu-central-1', 'ap-northeast-1', 'ap-south-1']
const DATACENTERS = ['iad1', 'pdx2', 'dub3', 'fra1', 'nrt2', 'bom1']
const DEVICES = ['ios', 'android', 'web', 'alexa', 'appletv', 'firetv']
const PAYMENT_METHODS = ['visa', 'mastercard', 'amex', 'paypal', 'gift_card', 'amazon_wallet']
const DECLINE_CODES = ['card_declined', 'insufficient_funds', 'lost_or_stolen', 'expired_card', 'amount_limit_exceeded']

// ---------------------------------------------------------------------------
// Event emitters — every one uses all LogRecord fields
// ---------------------------------------------------------------------------

function emit(event: {
  level: 'INFO' | 'WARN' | 'ERROR'
  module: string
  message: string
  body: Record<string, unknown>
}): void {
  const trace_id = newTraceId()
  greplog[event.level.toLowerCase() as 'info' | 'warn' | 'error'](
    event.message,
    {
      // correlation ids
      trace_id,
      request_id: `req_${trace_id.slice(2)}`,
      // service attribution
      service: `${SERVICE_GROUP}-${event.module}`,
      // the full domain payload
      ...event.body,
    },
  )
}

function emitSearch(): void {
  const user = pickList(USERS)
  const results = randomInt(0, 480)
  emit({
    level: results === 0 ? 'WARN' : 'INFO',
    module: 'search',
    message: results === 0 ? 'search returned no results' : 'search executed',
    body: {
      query: pickList(SEARCH_QUERIES),
      user_id: user.user_id,
      market: user.market,
      region: pickList(REGIONS),
      result_count: results,
      search_latency_ms: randomInt(40, 420),
      page: randomInt(1, 3),
      sort: pickList(['relevance', 'price-asc', 'price-desc', 'rating']),
      sponsored_slots: randomInt(0, 4),
    },
  })
}

function emitProductView(): void {
  const product = pickList(ASINS)
  emit({
    level: 'INFO',
    module: 'catalog',
    message: 'product detail served',
    body: {
      asin: product.asin,
      title: product.title,
      category: product.category,
      price: money(4.99, 249.99),
      currency: 'USD',
      user_id: pickList(USERS).user_id,
      region: pickList(REGIONS),
      datacenter: pickList(DATACENTERS),
      view_source: pickList(DEVICES),
      detail_latency_ms: randomInt(20, 260),
      cached: Math.random() < 0.7,
      stock_status: pick([
        ['in_stock', 12],
        ['low_stock', 3],
        ['out_of_stock', 1],
      ]) as string,
    },
  })
}

function emitAddToCart(): void {
  const product = pickList(ASINS)
  const quantity = randomInt(1, 5)
  emit({
    level: Math.random() < 0.04 ? 'WARN' : 'INFO',
    module: 'cart',
    message: Math.random() < 0.04 ? 'add-to-cart throttled' : 'item added to cart',
    body: {
      asin: product.asin,
      title: product.title,
      quantity,
      unit_price: money(2.0, 199.0),
      currency: 'USD',
      user_id: pickList(USERS).user_id,
      session_id: `sess_${newTraceId().slice(2, 12)}`,
      device: pickList(DEVICES),
      cart_size: randomInt(1, 25),
      region: pickList(REGIONS),
      datacenter: pickList(DATACENTERS),
    },
  })
}

function emitCheckout(): void {
  const checkout_id = `co_${newTraceId().slice(2)}`
  emit({
    level: 'INFO',
    module: 'checkout',
    message: 'checkout started',
    body: {
      checkout_id,
      user_id: pickList(USERS).user_id,
      item_count: randomInt(1, 12),
      subtotal: money(15.0, 1200.0),
      currency: 'USD',
      shipping_method: pickList(['standard', 'two-day', 'one-day', 'priority']),
      shipping_cost: money(0, 55),
      tax: money(1, 90),
      region: pickList(REGIONS),
      device: pickList(DEVICES),
    },
  })
}

function emitPayment(): void {
  const method = pickList(PAYMENT_METHODS)
  const amount = money(5, 1400)
  const decline = Math.random() < 0.08
  const fail = Math.random() < 0.02
  emit({
    level: decline || fail ? 'ERROR' : 'INFO',
    module: 'payments',
    message: decline ? 'payment declined' : fail ? 'payment provider timeout' : 'payment authorized',
    body: {
      payment_method: method,
      provider: pickList(['braintree', 'stripe', 'gc_payments', 'adyen']),
      amount,
      currency: 'USD',
      user_id: pickList(USERS).user_id,
      transaction_id: `txn_${newTraceId().slice(2)}`,
      authorization_ms: randomInt(180, 1400),
      retry_count: fail ? randomInt(1, 3) : 0,
      ...(decline ? { decline_code: pickList(DECLINE_CODES), decline_reason: 'issuer rejected transaction' } : {}),
      ...(fail ? { error: 'ECONNTIMEDOUT', provider_status: 'unavailable' } : {}),
    },
  })
}

function emitOrder(): void {
  const user = pickList(USERS)
  const fulfillment = pick([
    ['FBA', 4],
    ['MFN', 2],
    ['Prime Now', 1],
  ]) as string
  emit({
    level: 'INFO',
    module: 'orders',
    message: 'order placed successfully',
    body: {
      order_id: `ord_${newTraceId().slice(2)}`,
      user_id: user.user_id,
      account_type: user.account_type,
      item_count: randomInt(1, 15),
      order_total: money(10, 2500),
      currency: 'USD',
      payment_method: pickList(PAYMENT_METHODS),
      fulfillment_center: pick(['fc-amz-1', 'fc-amz-2', 'fc-amz-3', '3pl-west'] as const),
      fulfillment_channel: fulfillment,
      shipping_speed: pickList(['standard', 'priority']),
      region: user.market === 'US' ? pickList(['us-east-1', 'us-west-2']) : pickList(['eu-west-1', 'ap-northeast-1']),
      placed_at_latency_ms: randomInt(90, 620),
    },
  })
}

function emitInventory(): void {
  const product = pickList(ASINS)
  const level = pick([
    ['WARN', 5],
    ['ERROR', 1],
    ['INFO', 4],
  ]) as 'INFO' | 'WARN' | 'ERROR'
  emit({
    level,
    module: 'inventory',
    message: level === 'ERROR' ? 'inventory reconciliation failed' : level === 'WARN' ? 'inventory below reorder point' : 'inventory reorder triggered',
    body: {
      asin: product.asin,
      title: product.title,
      on_hand: randomInt(0, 900),
      committed: randomInt(0, 200),
      available: randomInt(0, 700),
      reorder_point: 120,
      warehouse: pickList(['BNA1', 'LAX9', 'FRA3', 'NRT5'] as const),
      ...(level === 'ERROR' ? { error: 'INVENTORY_API_5xx', error_code: 50002, source: 'inventory-service' } : {}),
    },
  })
}

function emitAuth(): void {
  const user = pickList(USERS)
  const outcome = pick([
    ['login_success', 4],
    ['mfa_challenge', 2],
    ['login_failed', 1],
  ]) as string
  emit({
    level: outcome === 'login_failed' ? 'WARN' : 'INFO',
    module: 'auth',
    message: outcome === 'login_failed' ? 'login failed' : outcome === 'mfa_challenge' ? 'MFA challenge issued' : 'login successful',
    body: {
      user_id: user.user_id,
      auth_provider: pickList(['cognito', 'internal-idp', 'social']),
      device: pickList(DEVICES),
      mfa_used: outcome === 'mfa_challenge',
      auth_latency_ms: randomInt(120, 900),
      source_ip: `${randomInt(1, 254)}.${randomInt(0, 254)}.${randomInt(0, 254)}.${randomInt(1, 254)}`,
      new_device: Math.random() < 0.15,
      ...(outcome === 'login_failed' ? { failure_reason: pickList(['wrong_password', 'account_locked', 'captcha_required']), attempts: randomInt(1, 5) } : {}),
    },
  })
}

function emitDownstreamCall(): void {
  const service = pickList(['inventory-service', 'price-service', 'recommendations', 'delivery-tracking', 'ratings-service'])
  const p99 = 350
  const latency = Math.round((Math.exp(-1.2 + Math.random() * 5) + Math.random() * 80) * 10) / 10
  const timeout = latency > 5000
  emit({
    level: timeout ? 'ERROR' : latency > p99 ? 'WARN' : 'INFO',
    module: 'edge',
    message: timeout ? `downstream timeout calling ${service}` : latency > p99 ? `slow downstream call to ${service}` : `downstream call to ${service} completed`,
    body: {
      dependency: service,
      latency_ms: Math.round(latency * 10) / 10,
      p99_ms: p99,
      http_status: timeout ? 504 : pickList(['200', '200', '200', '204', '301', '404']),
      retry_count: timeout ? randomInt(1, 2) : 0,
      region: pickList(REGIONS),
      datacenter: pickList(DATACENTERS),
      request_bytes: randomInt(120, 48_000),
      response_bytes: randomInt(300, 256_000),
    },
  })
}

function emitBackendHealth(): void {
  emit({
    level: Math.random() < 0.9 ? 'INFO' : 'WARN',
    module: 'operations',
    message: 'backend health report',
    body: {
      region: pickList(REGIONS),
      datacenter: pickList(DATACENTERS),
      active_instances: randomInt(12, 40),
      cpu_pct: randomInt(4, 91),
      memory_pct: randomInt(12, 88),
      rps_spike: Math.random() < 0.05,
      change_id: Math.random() < 0.3 ? `deploy_${randomInt(1000, 9999)}` : undefined,
    },
  })
}

// ---------------------------------------------------------------------------
// Event loop
// ---------------------------------------------------------------------------

const EMITTERS: Array<() => void> = [
  emitSearch,
  emitProductView,
  emitAddToCart,
  emitCheckout,
  emitPayment,
  emitOrder,
  emitInventory,
  emitAuth,
  emitDownstreamCall,
  emitBackendHealth,
]

// Bias toward the "customer journey" sequence without being strictly linear.
const SERIES = [
  emitSearch,
  emitProductView,
  emitAddToCart,
  emitAddToCart,
  emitCheckout,
  emitPayment,
  emitOrder,
]

setInterval(() => {
  // 70% of ticks follow the purchase funnel; 30% are ambient backend noise.
  const emitter = Math.random() < 0.7 ? pickList(SERIES) : pickList(EMITTERS)
  emitter()
}, Math.max(100, Math.round(1000 / RPS)))

greplog.init({
  service: SERVICE_GROUP,
  env: process.env.GREPLOG_ENV ?? 'development',
  endpoint: ENDPOINT,
})

console.log(
  `Greplog traffic generator running @ ${RPS} rps → ${ENDPOINT ?? 'http://127.0.0.1:5050'}, emitting Amazon backend events...`,
)
console.log('Press Ctrl+C to stop.')

function shutdown(signal: string): void {
  console.log(`\n${signal} received, flushing...`)
  void greplog.flush().finally(() => process.exit(0))
}

process.on('SIGINT', () => shutdown('SIGINT'))
process.on('SIGTERM', () => shutdown('SIGTERM'))