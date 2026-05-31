import React from 'react';
import styles from './styles.module.css';

const features = [
  {
    label: 'Open Source',
    icon: (
      <svg xmlns="http://www.w3.org/2000/svg" color='#6e47e4' width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
        <circle cx="12" cy="12" r="10"/>
        <path d="M12 2a10 10 0 0 1 0 20"/>
        <path d="M2 12h20"/>
        <path d="M12 2c2.5 2.5 4 6 4 10s-1.5 7.5-4 10"/>
      </svg>
    ),
  },
  {
    label: 'Modular',
    icon: (
      <svg xmlns="http://www.w3.org/2000/svg" color='#6e47e4' width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
        <rect x="3" y="3" width="7" height="7" rx="1"/>
        <rect x="14" y="3" width="7" height="7" rx="1"/>
        <rect x="3" y="14" width="7" height="7" rx="1"/>
        <rect x="14" y="14" width="7" height="7" rx="1"/>
      </svg>
    ),
  },
  {
    label: 'Composable',
    icon: (
      <svg xmlns="http://www.w3.org/2000/svg" color='#6e47e4' width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
        <path d="M12 2L2 7l10 5 10-5-10-5z"/>
        <path d="M2 17l10 5 10-5"/>
        <path d="M2 12l10 5 10-5"/>
      </svg>
    ),
  },
  {
    label: 'Secure',
    icon: (
      <svg xmlns="http://www.w3.org/2000/svg" color='#6e47e4' width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
        <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/>
      </svg>
    ),
  },
  {
    label: 'No Middlemen',
    icon: (
      <svg xmlns="http://www.w3.org/2000/svg" color='#6e47e4' width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
        <circle cx="12" cy="12" r="10"/>
        <path d="M4.93 4.93l14.14 14.14"/>
      </svg>
    ),
  },
  {
    label: 'Community Driven',
    icon: (
      <svg xmlns="http://www.w3.org/2000/svg" color='#6e47e4' width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
        <path d="M17 21v-2a4 4 0 0 0-4-4H5a4 4 0 0 0-4 4v2"/>
        <circle cx="9" cy="7" r="4"/>
        <path d="M23 21v-2a4 4 0 0 0-3-3.87"/>
        <path d="M16 3.13a4 4 0 0 1 0 7.75"/>
      </svg>
    ),
  },
];

export default function CompSec2() {
  return (
    <section className={styles.section}>
      <div className={styles.inner}>
        <p className={styles.label}>Built for Builders. Backed by Web3 Principles</p>

        <div className={styles.marqueeWrapper}>
          <div className={styles.marqueeTrack}>
            {[...features, ...features].map((f, i) => (
              <span key={i} className={styles.pill}>
                <span className={styles.pillIcon}>{f.icon}</span>
                {f.label}
              </span>
            ))}
          </div>
        </div>
      </div>
    </section>
  );
}