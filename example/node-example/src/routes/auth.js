import { Router } from 'express';

const router = Router();

router.post('/login', (req, res) => {
  const { email, password } = req.body || {};

  if (!email || !password) {
    console.warn('login failed: missing credentials', { email_provided: !!email });
    return res.status(400).json({ error: 'Email and password required' });
  }

  if (password !== 'secure_password_123') {
    console.warn('login failed: invalid password attempt', { email });
    return res.status(401).json({ error: 'Invalid credentials' });
  }

  console.info('login successful', { email, user_id: 'usr_98765' });
  res.json({ token: 'mock-jwt-token-789' });
});

export default router;