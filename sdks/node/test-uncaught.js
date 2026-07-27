
      const greplog = require('./dist/index.js');
      greplog.init({ tcpPort: 4319, serviceName: 'uncaught-test', socketPath: '/non/existent.sock' });
      
      setTimeout(() => {
        throw new Error('This is an uncaught exception');
      }, 100);
    