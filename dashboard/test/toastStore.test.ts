import { test } from 'node:test'
import assert from 'node:assert/strict'
import {
  createToastStore,
  DEFAULT_ERROR_DURATION_MS,
  DEFAULT_SUCCESS_DURATION_MS,
  ERROR_COOLDOWN_MS,
  type ToastStoreDeps,
} from '../src/lib/toastStore.ts'

function makeFakeDeps(): ToastStoreDeps & { advance: (ms: number) => void; time: () => number } {
  let time = 0
  let nextId = 0
  const timers = new Map<number, { fn: () => void; at: number }>()

  const runDue = () => {
    const due = [...timers.entries()]
      .filter(([, t]) => t.at <= time)
      .sort((a, b) => a[1].at - b[1].at)
    for (const [id, t] of due) {
      timers.delete(id)
      t.fn()
    }
  }

  return {
    now: () => time,
    schedule: (fn, ms) => {
      const id = ++nextId
      timers.set(id, { fn, at: time + ms })
      return id
    },
    cancel: (id) => {
      timers.delete(id)
    },
    advance: (ms) => {
      time += ms
      runDue()
    },
    time: () => time,
  }
}

test('error without dedupeKey always shows (user-initiated action)', () => {
  const store = createToastStore(makeFakeDeps())
  store.showError('first')
  store.showError('second')
  assert.equal(store.getToasts().length, 2)
})

test('success without dedupeKey always shows (explicit action outcome)', () => {
  const store = createToastStore(makeFakeDeps())
  store.showSuccess('exported')
  store.showSuccess('cleared')
  assert.equal(store.getToasts().length, 2)
})

test('identical error with dedupeKey is suppressed while already showing', () => {
  const store = createToastStore(makeFakeDeps())
  store.showError('Query failed', { dedupeKey: 'query-error:/query' })
  store.showError('Query failed', { dedupeKey: 'query-error:/query' })
  store.showError('Query failed', { dedupeKey: 'query-error:/query' })
  assert.equal(store.getToasts().length, 1)
})

test('deduped repeat does not reset the dismiss timer (ignore-repeat policy)', () => {
  const deps = makeFakeDeps()
  const store = createToastStore(deps)
  store.showError('Query failed', { dedupeKey: 'query-error:/query' })
  deps.advance(DEFAULT_ERROR_DURATION_MS - 1)
  store.showError('Query failed', { dedupeKey: 'query-error:/query' })
  assert.equal(store.getToasts().length, 1)
  deps.advance(1)
  assert.equal(store.getToasts().length, 0)
})

test('ongoing failure re-toasts at most once per cooldown window, not per poll', () => {
  const deps = makeFakeDeps()
  const store = createToastStore(deps)
  store.showError('Query failed', { dedupeKey: 'query-error:/query' })
  assert.equal(store.getToasts().length, 1)

  deps.advance(DEFAULT_ERROR_DURATION_MS) // toast expires
  assert.equal(store.getToasts().length, 0)

  store.showError('Query failed', { dedupeKey: 'query-error:/query' }) // within cooldown
  assert.equal(store.getToasts().length, 0)

  deps.advance(ERROR_COOLDOWN_MS) // cooldown elapses
  store.showError('Query failed', { dedupeKey: 'query-error:/query' })
  assert.equal(store.getToasts().length, 1)
})

test('success with a dedupeKey that is not in an error state is suppressed (routine success)', () => {
  const deps = makeFakeDeps()
  const store = createToastStore(deps)
  store.showSuccess('Query succeeded again', { dedupeKey: 'query-error:/query' })
  assert.equal(store.getToasts().length, 0)
  deps.advance(DEFAULT_SUCCESS_DURATION_MS)
  assert.equal(store.getToasts().length, 0)
})

test('recovery: success with a dedupeKey clears the error and shows the recovery toast', () => {
  const deps = makeFakeDeps()
  const store = createToastStore(deps)
  store.showError('Query failed', { dedupeKey: 'query-error:/query' })
  assert.equal(store.getToasts().length, 1)
  assert.equal(store.getToasts()[0]?.variant, 'error')

  store.showSuccess('Query succeeded again', { dedupeKey: 'query-error:/query' })
  const toasts = store.getToasts()
  assert.equal(toasts.length, 1)
  assert.equal(toasts[0]?.variant, 'success')
  assert.equal(toasts[0]?.message, 'Query succeeded again')
})

test('after recovery, a new failure is treated as a fresh event (no stale cooldown)', () => {
  const deps = makeFakeDeps()
  const store = createToastStore(deps)
  store.showError('Query failed', { dedupeKey: 'query-error:/query' })
  store.showSuccess('Query succeeded again', { dedupeKey: 'query-error:/query' })
  deps.advance(DEFAULT_SUCCESS_DURATION_MS)
  assert.equal(store.getToasts().length, 0)

  store.showError('Query failed', { dedupeKey: 'query-error:/query' })
  assert.equal(store.getToasts().length, 1)
})

test('persistent error (durationMs 0) never auto-dismisses', () => {
  const deps = makeFakeDeps()
  const store = createToastStore(deps)
  store.showError('Agent unreachable', { dedupeKey: 'agent-connection', durationMs: 0 })
  assert.equal(store.getToasts().length, 1)
  deps.advance(ERROR_COOLDOWN_MS * 10)
  assert.equal(store.getToasts().length, 1)
})

test('recovery dismisses a persistent error toast', () => {
  const deps = makeFakeDeps()
  const store = createToastStore(deps)
  store.showError('Agent unreachable', { dedupeKey: 'agent-connection', durationMs: 0 })
  assert.equal(store.getToasts()[0]?.variant, 'error')

  store.showSuccess('Reconnected to agent', { dedupeKey: 'agent-connection' })
  const toasts = store.getToasts()
  assert.equal(toasts.length, 1)
  assert.equal(toasts[0]?.variant, 'success')
})

test('manual dismiss removes a toast', () => {
  const deps = makeFakeDeps()
  const store = createToastStore(deps)
  store.showError('Query failed', { dedupeKey: 'query-error:/query' })
  const id = store.getToasts()[0]?.id
  assert.ok(id)
  store.dismiss(id)
  assert.equal(store.getToasts().length, 0)
})

test('auto-dismiss happens after the variant default duration', () => {
  const deps = makeFakeDeps()
  const store = createToastStore(deps)
  store.showError('Query failed', { dedupeKey: 'query-error:/query' })
  store.showSuccess('exported')
  assert.equal(store.getToasts().length, 2)

  deps.advance(DEFAULT_SUCCESS_DURATION_MS)
  assert.equal(store.getToasts().length, 1)
  assert.equal(store.getToasts()[0]?.variant, 'error')

  deps.advance(DEFAULT_ERROR_DURATION_MS - DEFAULT_SUCCESS_DURATION_MS)
  assert.equal(store.getToasts().length, 0)
})

test('subscribe fires on add and dismiss, snapshot is stable between mutations', () => {
  const store = createToastStore(makeFakeDeps())
  let calls = 0
  store.subscribe(() => {
    calls += 1
  })

  const empty = store.getToasts()
  store.showError('Query failed', { dedupeKey: 'query-error:/query' })
  assert.equal(store.getToasts() === empty, false)
  assert.equal(store.getToasts() === store.getToasts(), true)

  const id = store.getToasts()[0]?.id
  assert.ok(id)
  store.dismiss(id)
  assert.ok(calls >= 2)
})
