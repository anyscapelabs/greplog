// 1. Initialize Greplog FIRST
import greplog from 'greplog';

greplog.init({
  service: 'api-gateway',
  env: 'development'
});

import express from 'express';
import authRoutes from './routes/auth.js';
import orderRoutes from './routes/orders.js';

const app = express();
app.use(express.json());

// 2. Global HTTP Request Logger
app.use((req, res, next) => {
  const start = Date.now();
  res.on('finish', () => {
    const duration = Date.now() - start;
    // Captured automatically by Greplog
    console.info(`HTTP ${req.method} ${req.path} status=${res.statusCode} duration=${duration}ms ip=${req.ip}`);
  });
  next();
});

// 3. Mount Routes
app.use('/api/auth', authRoutes);
app.use('/api/orders', orderRoutes);

// 4. Start the Server
const PORT = process.env.PORT || 4000;
app.listen(PORT, () => {
  console.log(`Server running on port ${PORT}`);
  console.log(`Greplog auto-instrumentation active.`);
});