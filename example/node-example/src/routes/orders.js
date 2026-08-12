import { Router } from 'express';

const router = Router();

router.post('/', (req, res) => {
  const { items, total_price, user_id } = req.body || {};

  try {
    if (!items || items.length === 0) {
      throw new Error('Order must contain at least one item');
    }

    const orderId = `ord_${Math.random().toString(36).substring(7)}`;

    console.info('order processed successfully', {
      order_id: orderId,
      user_id,
      item_count: items.length,
      revenue: total_price
    });

    res.status(201).json({ order_id: orderId, status: 'success' });

  } catch (error) {
    // Greplog extracts the stack trace automatically
    console.error('failed to process order', error);
    res.status(400).json({ error: error.message });
  }
});

export default router;