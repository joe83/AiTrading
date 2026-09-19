import { useState, useEffect } from 'react';
import './LiveClock.css';

export function LiveClock() {
  const [time, setTime] = useState(new Date());

  useEffect(() => {
    const timer = setInterval(() => setTime(new Date()), 1000);
    return () => clearInterval(timer);
  }, []);

  const utc = time.toISOString().slice(11, 19);
  const local = time.toLocaleTimeString('en-US', { hour12: false });

  return (
    <div className="live-clock">
      <span className="live-clock__utc">{utc} <span className="live-clock__tz">UTC</span></span>
      <span className="live-clock__local">{local} <span className="live-clock__tz">LOCAL</span></span>
    </div>
  );
}
