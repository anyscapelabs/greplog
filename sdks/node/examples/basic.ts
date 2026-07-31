import { greplog } from 'greplog';

// 1. Auto-capture only — captures uncaught exceptions, unhandled rejections, and HTTP metadata automatically
greplog.init();

// 2. Service name override
greplog.init({ service: 'api-backend' });

// Fail-open guarantee: these calls are safe even if the agent isn't running
// 3. Manual log levels with structured details
greplog.error('Payment failed', { orderId: 'ord-123', amount: '99.99' });
greplog.warn('Retrying request', { attempt: '2' });
greplog.info('Server started', { port: '4000' });
greplog.debug('Cache miss', { key: 'user:123' });
