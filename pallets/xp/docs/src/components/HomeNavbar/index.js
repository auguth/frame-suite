import React, { useState, useEffect } from 'react';
import { useNavbarMobileSidebar } from '@docusaurus/theme-common/internal';
import Link from '@docusaurus/Link';
import { useLocation } from '@docusaurus/router';
import styles from './styles.module.css';

export default HomeNavbar;

function HomeNavbar() {
  const [scrolled, setScrolled] = useState(false);
  const [menuOpen, setMenuOpen] = useState(false);
  const [stars, setStars]       = useState(null);
  const { pathname }            = useLocation();
  const mobileSidebar           = useNavbarMobileSidebar();

  const isDocs = pathname.includes('/docs');

  useEffect(() => {
    const onScroll = () => setScrolled(window.scrollY > 20);
    window.addEventListener('scroll', onScroll);
    return () => window.removeEventListener('scroll', onScroll);
  }, []);

  useEffect(() => {
    fetch('https://api.github.com/repos/auguth/frame-suite')
      .then((r) => r.json())
      .then((d) => { if (d.stargazers_count !== undefined) setStars(d.stargazers_count); })
      .catch(() => {});
  }, []);

  const formatStars = (n) => {
    if (n === null) return null;
    if (n >= 1000) return `${(n / 1000).toFixed(1)}k`;
    return String(n);
  };

  const handleSidebarToggle = () => {
    setMenuOpen(false);
    mobileSidebar.toggle();
  };

  const handleMenuToggle = () => {
    if (mobileSidebar.shown) mobileSidebar.toggle();
    setMenuOpen((v) => !v);
  };

  return (
    <nav className={`${styles.navbar} ${scrolled ? styles.scrolled : ''} ${isDocs ? styles.navbarDocs : ''}`}>
      <div className={`${styles.inner} ${isDocs ? styles.innerDocs : ''}`}>

        {isDocs && (
          <button
            className={`${styles.sidebarToggle} ${styles.sidebarToggleMobile}`}
            onClick={handleSidebarToggle}
            aria-label={mobileSidebar.shown ? 'Close sidebar' : 'Open sidebar'}
          >
            {mobileSidebar.shown ? (
              <svg width="18" height="18" viewBox="0 0 24 24" fill="none"
                stroke="currentColor" strokeWidth="2.2" strokeLinecap="round">
                <line x1="18" y1="6" x2="6" y2="18"/>
                <line x1="6" y1="6" x2="18" y2="18"/>
              </svg>
            ) : (
              <svg width="18" height="18" viewBox="0 0 24 24" fill="none"
                stroke="currentColor" strokeWidth="2.2" strokeLinecap="round">
                <rect x="3" y="3" width="7" height="18" rx="1"/>
                <line x1="14" y1="8" x2="21" y2="8"/>
                <line x1="14" y1="12" x2="21" y2="12"/>
                <line x1="14" y1="16" x2="21" y2="16"/>
              </svg>
            )}
          </button>
        )}

        <Link to="/" className={`${styles.logo} ${isDocs ? styles.logoCentered : ''}`}>
          <span className={styles.logoText}>
            Pallet - <span className={styles.logoAccent}>XP</span>
          </span>
        </Link>

        <ul className={styles.navLinks}>
          <li>
            <Link to="/docs/intro" className={styles.navLink}>
              <svg className={styles.navIcon} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
                <path d="M4 19.5A2.5 2.5 0 0 1 6.5 17H20"/>
                <path d="M6.5 2H20v20H6.5A2.5 2.5 0 0 1 4 19.5v-15A2.5 2.5 0 0 1 6.5 2z"/>
              </svg>
              Docs
            </Link>
          </li>
          <li>
            <a href="https://crates.io/crates/pallet-xp" className={styles.navLink} target="_blank" rel="noopener noreferrer">
              <svg className={styles.navIcon} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
                <path d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z"/>
                <polyline points="3.27 6.96 12 12.01 20.73 6.96"/>
                <line x1="12" y1="22.08" x2="12" y2="12"/>
              </svg>
              Crates
            </a>
          </li>
          <li>
            <a href="https://docs.rs/pallet-xp/0.1.1/src/pallet_xp/lib.rs.html#16-2537" className={styles.navLink} target="_blank" rel="noopener noreferrer">
              <svg className={styles.navIcon} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
                <polyline points="16 18 22 12 16 6"/>
                <polyline points="8 6 2 12 8 18"/>
              </svg>
              Source
            </a>
          </li>
          <li>
            <a href="https://github.com/auguth/frame-suite/tree/master/pallets/xp" className={styles.navLink} target="_blank" rel="noopener noreferrer">
              <svg className={styles.navIcon} viewBox="0 0 24 24" fill="currentColor">
                <path d="M12 2C6.477 2 2 6.484 2 12.017c0 4.425 2.865 8.18 6.839 9.504.5.092.682-.217.682-.483 0-.237-.008-.868-.013-1.703-2.782.605-3.369-1.343-3.369-1.343-.454-1.158-1.11-1.466-1.11-1.466-.908-.62.069-.608.069-.608 1.003.07 1.531 1.032 1.531 1.032.892 1.53 2.341 1.088 2.91.832.092-.647.35-1.088.636-1.338-2.22-.253-4.555-1.113-4.555-4.951 0-1.093.39-1.988 1.029-2.688-.103-.253-.446-1.272.098-2.65 0 0 .84-.27 2.75 1.026A9.564 9.564 0 0 1 12 6.844a9.59 9.59 0 0 1 2.504.337c1.909-1.296 2.747-1.027 2.747-1.027.546 1.379.202 2.398.1 2.651.64.7 1.028 1.595 1.028 2.688 0 3.848-2.339 4.695-4.566 4.943.359.309.678.92.678 1.855 0 1.338-.012 2.419-.012 2.747 0 .268.18.58.688.482A10.02 10.02 0 0 0 22 12.017C22 6.484 17.522 2 12 2z"/>
              </svg>
              Repo
            </a>
          </li>
          <li>
            <a href="https://github.com/auguth/xp-substrate-template" className={styles.navLink} target="_blank" rel="noopener noreferrer">
              <svg className={styles.navIcon} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
                <rect x="3" y="3" width="7" height="7"/>
                <rect x="14" y="3" width="7" height="7"/>
                <rect x="14" y="14" width="7" height="7"/>
                <rect x="3" y="14" width="7" height="7"/>
              </svg>
              Template
            </a>
          </li>
        </ul>

        <div className={styles.rightGroup}>
          <div className={styles.navSep} />
          <a href="https://github.com/auguth/frame-suite" className={styles.ghBtn}
            target="_blank" rel="noopener noreferrer" aria-label="Star on GitHub">
            <svg className={styles.ghIcon} viewBox="0 0 24 24" fill="currentColor">
              <path d="M12 2C6.477 2 2 6.484 2 12.017c0 4.425 2.865 8.18 6.839 9.504.5.092.682-.217.682-.483 0-.237-.008-.868-.013-1.703-2.782.605-3.369-1.343-3.369-1.343-.454-1.158-1.11-1.466-1.11-1.466-.908-.62.069-.608.069-.608 1.003.07 1.531 1.032 1.531 1.032.892 1.53 2.341 1.088 2.91.832.092-.647.35-1.088.636-1.338-2.22-.253-4.555-1.113-4.555-4.951 0-1.093.39-1.988 1.029-2.688-.103-.253-.446-1.272.098-2.65 0 0 .84-.27 2.75 1.026A9.564 9.564 0 0 1 12 6.844a9.59 9.59 0 0 1 2.504.337c1.909-1.296 2.747-1.027 2.747-1.027.546 1.379.202 2.398.1 2.651.64.7 1.028 1.595 1.028 2.688 0 3.848-2.339 4.695-4.566 4.943.359.309.678.92.678 1.855 0 1.338-.012 2.419-.012 2.747 0 .268.18.58.688.482A10.02 10.02 0 0 0 22 12.017C22 6.484 17.522 2 12 2z"/>
            </svg>
            Star
            <span className={styles.starBadge}>
              <svg className={styles.starIcon} viewBox="0 0 24 24" fill="currentColor">
                <path d="M12 2l3.09 6.26L22 9.27l-5 4.87 1.18 6.88L12 17.77l-6.18 3.25L7 14.14 2 9.27l6.91-1.01L12 2z"/>
              </svg>
              {stars !== null ? formatStars(stars) : ''}
            </span>
          </a>
          <Link to="/docs/getting-started/installation" className={styles.ctaButton}>
            Get Started
          </Link>
        </div>

        <button
          className={styles.hamburger}
          onClick={handleMenuToggle}
          aria-label="Toggle menu"
        >
          <span className={`${styles.bar} ${menuOpen ? styles.barTop : ''}`} />
          <span className={`${styles.bar} ${menuOpen ? styles.barMid : ''}`} />
          <span className={`${styles.bar} ${menuOpen ? styles.barBot : ''}`} />
        </button>

      </div>

      <div className={`${styles.mobileMenu} ${menuOpen ? styles.mobileMenuOpen : ''}`}>

        <Link to="/docs/intro" className={styles.mobileLink} onClick={() => setMenuOpen(false)}>
          <svg className={styles.mobileLinkIcon} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
            <path d="M4 19.5A2.5 2.5 0 0 1 6.5 17H20"/>
            <path d="M6.5 2H20v20H6.5A2.5 2.5 0 0 1 4 19.5v-15A2.5 2.5 0 0 1 6.5 2z"/>
          </svg>
          Docs
        </Link>

        <a href="https://crates.io/crates/pallet-xp" className={styles.mobileLink} onClick={() => setMenuOpen(false)}>
          <svg className={styles.mobileLinkIcon} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
            <path d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z"/>
            <polyline points="3.27 6.96 12 12.01 20.73 6.96"/>
            <line x1="12" y1="22.08" x2="12" y2="12"/>
          </svg>
          Crates
        </a>

        <a href="https://docs.rs/pallet-xp/0.1.1/src/pallet_xp/lib.rs.html#16-2537" className={styles.mobileLink} onClick={() => setMenuOpen(false)}>
          <svg className={styles.mobileLinkIcon} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
            <polyline points="16 18 22 12 16 6"/>
            <polyline points="8 6 2 12 8 18"/>
          </svg>
          Source
        </a>

        <a href="https://github.com/auguth/frame-suite" className={styles.mobileLink} onClick={() => setMenuOpen(false)}>
          <svg className={styles.mobileLinkIcon} viewBox="0 0 24 24" fill="currentColor">
            <path d="M12 2C6.477 2 2 6.484 2 12.017c0 4.425 2.865 8.18 6.839 9.504.5.092.682-.217.682-.483 0-.237-.008-.868-.013-1.703-2.782.605-3.369-1.343-3.369-1.343-.454-1.158-1.11-1.466-1.11-1.466-.908-.62.069-.608.069-.608 1.003.07 1.531 1.032 1.531 1.032.892 1.53 2.341 1.088 2.91.832.092-.647.35-1.088.636-1.338-2.22-.253-4.555-1.113-4.555-4.951 0-1.093.39-1.988 1.029-2.688-.103-.253-.446-1.272.098-2.65 0 0 .84-.27 2.75 1.026A9.564 9.564 0 0 1 12 6.844a9.59 9.59 0 0 1 2.504.337c1.909-1.296 2.747-1.027 2.747-1.027.546 1.379.202 2.398.1 2.651.64.7 1.028 1.595 1.028 2.688 0 3.848-2.339 4.695-4.566 4.943.359.309.678.92.678 1.855 0 1.338-.012 2.419-.012 2.747 0 .268.18.58.688.482A10.02 10.02 0 0 0 22 12.017C22 6.484 17.522 2 12 2z"/>
          </svg>
          Repo
        </a>

        <a href="https://github.com/auguth/xp-substrate-template" className={styles.mobileLink} onClick={() => setMenuOpen(false)}>
          <svg className={styles.mobileLinkIcon} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
            <rect x="3" y="3" width="7" height="7"/>
            <rect x="14" y="3" width="7" height="7"/>
            <rect x="14" y="14" width="7" height="7"/>
            <rect x="3" y="14" width="7" height="7"/>
          </svg>
          Template
        </a>

        <div className={styles.mobileCtas}>
          <a href="https://github.com/auguth/frame-suite" className={styles.mobileGhBtn}
            target="_blank" rel="noopener noreferrer">
            <svg className={styles.mobileLinkIcon} viewBox="0 0 24 24" fill="currentColor">
              <path d="M12 2C6.477 2 2 6.484 2 12.017c0 4.425 2.865 8.18 6.839 9.504.5.092.682-.217.682-.483 0-.237-.008-.868-.013-1.703-2.782.605-3.369-1.343-3.369-1.343-.454-1.158-1.11-1.466-1.11-1.466-.908-.62.069-.608.069-.608 1.003.07 1.531 1.032 1.531 1.032.892 1.53 2.341 1.088 2.91.832.092-.647.35-1.088.636-1.338-2.22-.253-4.555-1.113-4.555-4.951 0-1.093.39-1.988 1.029-2.688-.103-.253-.446-1.272.098-2.65 0 0 .84-.27 2.75 1.026A9.564 9.564 0 0 1 12 6.844a9.59 9.59 0 0 1 2.504.337c1.909-1.296 2.747-1.027 2.747-1.027.546 1.379.202 2.398.1 2.651.64.7 1.028 1.595 1.028 2.688 0 3.848-2.339 4.695-4.566 4.943.359.309.678.92.678 1.855 0 1.338-.012 2.419-.012 2.747 0 .268.18.58.688.482A10.02 10.02 0 0 0 22 12.017C22 6.484 17.522 2 12 2z"/>
            </svg>
            Star
            <span className={styles.mobileStarBadge}>
              <svg viewBox="0 0 24 24" fill="currentColor" style={{width:11,height:11}}>
                <path d="M12 2l3.09 6.26L22 9.27l-5 4.87 1.18 6.88L12 17.77l-6.18 3.25L7 14.14 2 9.27l6.91-1.01L12 2z"/>
              </svg>
              {stars !== null ? formatStars(stars) : ''}
            </span>
          </a>
          <Link to="/docs/getting-started/installation" className={styles.mobileCta}
            onClick={() => setMenuOpen(false)}>
            Get Started
          </Link>
        </div>

      </div>
    </nav>
  );
}