import React from 'react';
import Link from '@docusaurus/Link';
import styles from './styles.module.css';

const logoMark  = require('@site/static/img/xp-badge.png').default;
const auguthImg = require('@site/static/img/parent_logo_v2.png').default;

const links = [
  { label: 'Github',   href: 'https://github.com/auguth/' },
  { label: 'Docs',   to: '/docs/intro', internal: true },
  { label: 'Crates', href: 'https://crates.io/crates/pallet-xp' },
  { label: 'Source', href: 'https://docs.rs/pallet-xp/0.1.1/src/pallet_xp/lib.rs.html#16-2537' },
  { label: 'Template', href: 'https://github.com/auguth/xp-substrate-template' },
];

const social = [
  { label: 'LinkedIn', href: 'https://www.linkedin.com/company/auguth-labs/' },
  { label: 'Email',    href: 'mailto:auguthlabs@gmail.com' },
  { label: 'Discussions',    href: 'https://github.com/auguth/frame-suite/discussions' },
  { label: 'Matrix',    href: 'https://matrix.to/#/#auguthlabs:matrix.org' },
  { label: 'Twitter',    href: 'https://twitter.com/auguthlab' },
];

const LicLogo = () => (
  <svg class="w-6 h-6 text-gray-800 dark:text-white" aria-hidden="true" xmlns="http://www.w3.org/2000/svg" width="20" height="20" fill="none" viewBox="0 0 24 24">
    <path stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5.5 21h13M12 21V7m0 0a2 2 0 1 0 0-4 2 2 0 0 0 0 4Zm2-1.8c3.073.661 2.467 2.8 5 2.8M5 8c3.359 0 2.192-2.115 5.012-2.793M7 9.556V7.75m0 1.806-1.95 4.393a.773.773 0 0 0 .37.962.785.785 0 0 0 .362.089h2.436a.785.785 0 0 0 .643-.335.776.776 0 0 0 .09-.716L7 9.556Zm10 0V7.313m0 2.243-1.95 4.393a.773.773 0 0 0 .37.962.786.786 0 0 0 .362.089h2.436a.785.785 0 0 0 .643-.335.775.775 0 0 0 .09-.716L17 9.556Z"/>
  </svg>
);

const BuiltLogo = () => (
  <svg class="w-6 h-6 text-gray-800 dark:text-white" aria-hidden="true" xmlns="http://www.w3.org/2000/svg" width="20" height="20" fill="none" viewBox="0 0 24 24">
    <path stroke="currentColor" stroke-linejoin="round" stroke-width="2" d="M7.58209 8.96025 9.8136 11.1917l-1.61782 1.6178c-1.08305-.1811-2.23623.1454-3.07364.9828-1.1208 1.1208-1.32697 2.8069-.62368 4.1363.14842.2806.42122.474.73509.5213.06726.0101.1347.0133.20136.0098-.00351.0666-.00036.1341.00977.2013.04724.3139.24069.5867.52125.7351 1.32944.7033 3.01552.4971 4.13627-.6237.8375-.8374 1.1639-1.9906.9829-3.0736l4.8107-4.8108c1.0831.1811 2.2363-.1454 3.0737-.9828 1.1208-1.1208 1.3269-2.80688.6237-4.13632-.1485-.28056-.4213-.474-.7351-.52125-.0673-.01012-.1347-.01327-.2014-.00977.0035-.06666.0004-.13409-.0098-.20136-.0472-.31386-.2406-.58666-.5212-.73508-1.3294-.70329-3.0155-.49713-4.1363.62367-.8374.83741-1.1639 1.9906-.9828 3.07365l-1.7788 1.77875-2.23152-2.23148-1.41419 1.41424Zm1.31056-3.1394c-.04235-.32684-.24303-.61183-.53647-.76186l-1.98183-1.0133c-.38619-.19746-.85564-.12345-1.16234.18326l-.86321.8632c-.3067.3067-.38072.77616-.18326 1.16235l1.0133 1.98182c.15004.29345.43503.49412.76187.53647l1.1127.14418c.3076.03985.61628-.06528.8356-.28461l.86321-.8632c.21932-.21932.32446-.52801.2846-.83561l-.14417-1.1127ZM19.4448 16.4052l-3.1186-3.1187c-.7811-.781-2.0474-.781-2.8285 0l-.1719.172c-.7811.781-.7811 2.0474 0 2.8284l3.1186 3.1187c.7811.781 2.0474.781 2.8285 0l.1719-.172c.7811-.781.7811-2.0474 0-2.8284Z"/>
  </svg>
);

const LinkIcon = () => (
  <svg class="w-6 h-6 text-gray-800 dark:text-white" aria-hidden="true" xmlns="http://www.w3.org/2000/svg" width="12" height="12" fill="none" viewBox="0 0 24 24">
    <path stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13.213 9.787a3.391 3.391 0 0 0-4.795 0l-3.425 3.426a3.39 3.39 0 0 0 4.795 4.794l.321-.304m-.321-4.49a3.39 3.39 0 0 0 4.795 0l3.424-3.426a3.39 3.39 0 0 0-4.794-4.795l-1.028.961"/>
  </svg>
);

const SocialIcon = () => (
  <svg viewBox="0 0 24 24" width="12" height="12" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
    <circle cx="18" cy="5" r="3"/>
    <circle cx="6" cy="12" r="3"/>
    <circle cx="18" cy="19" r="3"/>
    <line x1="8.59" y1="13.51" x2="15.42" y2="17.49"/>
    <line x1="15.41" y1="6.51" x2="8.59" y2="10.49"/>
  </svg>
) 

const AuguthBlock = () => (
  <div className={styles.auguthBlock}>
    <div className={styles.auguthImgBox}>
      <img src={auguthImg} alt="Auguth Labs" className={styles.auguthImg} />
    </div>
    <p className={styles.auguthSub}>Part of Auguth Labs<br/>open-source softwares.</p>
    <p className={styles.auguthSub}>Built for the Substrate<br/>framework.</p>
  </div>
);

const LinksBlock = () => (
  <div className={styles.linksBlock}>
    <div className={styles.col}>
      <p className={styles.colHeading}><span className={styles.fIcon}><LinkIcon/></span>Links</p>
      {links.map((l) =>
        l.internal ? (
          <Link
            key={l.label}
            to={l.to}
            className={styles.colLink}
          >
            {l.label}
          </Link>
        ) : (
          <a
            key={l.label}
            href={l.href}
            className={styles.colLink}
            target="_blank"
            rel="noopener noreferrer"
          >
            {l.label}
          </a>
        )
      )}
    </div>
    <div className={styles.col}>
      <p className={styles.colHeading}><span className={styles.fIcon}><SocialIcon/></span>Social</p>
      {social.map((s) => (
        <a key={s.label} href={s.href} className={styles.colLink}
          target="_blank" rel="noopener noreferrer">{s.label}</a>
      ))}
    </div>
  </div>
);

export default function HomeFooter() {
  return (
    <footer className={styles.footer}>
      <div className={styles.box}>

        <div className={styles.desktopRow}>

          <div className={styles.leftBlock}>
            <div className={styles.pxpBlock}>
              <div className={styles.brandRow}>
                <div className={styles.logoMark}>
                  <img src={logoMark} alt="Pallet-XP" className={styles.logoMarkImg} />
                </div>
                <span className={styles.brandName}>Pallet-XP</span>
              </div>
              <p className={styles.tagline}>
                A reputation-driven XP system for tracking<br/> contribution, consistency, and participation <br/>in non-trusted runtime environments.
              </p>
              <a
              href="https://www.mozilla.org/en-US/MPL/2.0/"
              className={styles.licBadge}
              target="_blank"
              rel="noopener noreferrer"
              >
                <LicLogo />
                Mozila Public Licence 2.0
              </a>
            </div>
            <div className={styles.innerVsep} />
            <LinksBlock />
          </div>

          <div className={styles.outerVsep} />

          <div className={styles.desktopAuguth}>
            <AuguthBlock />
          </div>
        </div>

        <div className={styles.tabletLayout}>
          <div className={styles.tabletTop}>
            <div className={styles.brandRow}>
              <div className={styles.logoMark}>
                <img src={logoMark} alt="Pallet-XP" className={styles.logoMarkImg} />
              </div>
              <span className={styles.brandName}>Pallet-XP</span>
            </div>
            <p className={styles.tagline}>
              A reputation-driven XP system for tracking<br/> contribution, consistency, and participation <br/>in non-trusted runtime environments.
            </p>
            <a
            href="https://www.mozilla.org/en-US/MPL/2.0/"
            className={styles.licBadge}
            target="_blank"
            rel="noopener noreferrer"
            >
              <LicLogo />
              Mozila Public Licence 2.0
            </a>
          </div>
          <div className={styles.hdivider} />
          <div className={styles.tabletBottom}>
            <AuguthBlock />
            <div className={styles.outerVsep} />
            <LinksBlock />
          </div>
        </div>

        <div className={styles.mobileLayout}>
          <div className={styles.brandRow}>
            <div className={styles.logoMark}>
              <img src={logoMark} alt="Pallet-XP" className={styles.logoMarkImg} />
            </div>
            <span className={styles.brandName}>Pallet-XP</span>
          </div>
          <p className={styles.tagline}>
            A reputation-driven XP system for tracking<br/> contribution, consistency, and participation <br/>in non-trusted runtime environments.
          </p>
          <a
          href="https://www.mozilla.org/en-US/MPL/2.0/"
          className={styles.licBadge}
          target="_blank"
          rel="noopener noreferrer"
          >
            <LicLogo />
            Mozila Public Licence 2.0
          </a>
          <div className={styles.hdivider} />
          <LinksBlock />
          <div className={styles.hdivider} />
          <AuguthBlock />
        </div>

        <div className={styles.bottom}>
          <p className={styles.copy}>(C) 2026 Auguth Labs (OPC) Pvt Ltd, India. All rights reserved.</p>
        </div>

      </div>
    </footer>
  );
}