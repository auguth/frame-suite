import React from 'react';
import styles from './styles.module.css';
import Link from '@docusaurus/Link';
const mascotImg = require('@site/static/img/CompSec-8-1.png').default;

const Kw = ({ children }) => (
  <code style={{
    fontFamily: "'Fira Code', monospace",
    fontSize: '0.82em',
    background: 'rgba(127, 95, 255, 0.1)',
    border: '1px solid rgba(127, 95, 255, 0.2)',
    borderRadius: 4,
    padding: '0.1em 0.35em',
    color: '#c4b5fd',
  }}>{children}</code>
);

const leftFeatures = [
  { text: 'Modular pallet',         desc: "Plug in what you need. Skip what you don't.",    color: '#4ade80' },
  { text: 'Composable by design',   desc: 'Traits and hooks for any runtime.',               color: '#4ade80' },
  { text: 'No std assumptions',     desc: <>Works in <Kw>no_std + WASM</Kw> environments.</>,            color: '#4ade80' },
  { text: 'Well-documented',        desc: 'Inline docs, examples, and guides included.',     color: '#4ade80' },
  { text: 'Fully on-chain',         desc: 'No oracles. No bridges. Pure on-chain logic.',   color: '#4ade80' },
];

const rightFeatures = [
  { text: 'Key-based identity',    desc: <>Each <Kw>XpId</Kw> is an on-chain reputation slot owned by an account.</>, color: '#818cf8' },
  { text: 'Non-monetary design',   desc: 'No tokens. No inflation. Pure reputation.',        color: '#818cf8' },
  { text: 'Reputation-first',      desc: 'Every mechanic is built around earning XP.',       color: '#818cf8' },
  { text: 'Extensible via traits', desc: 'Hook into XP events from any pallet.',            color: '#818cf8' },
  { text: 'Upgrade-friendly',      desc: 'Safe storage migrations across versions.',         color: '#818cf8' },
];

const FeatureList = ({ features }) => (
  <ul className={styles.featureList}>
    {features.map((f, i) => (
      <li key={i} className={styles.feature}>
        <span className={styles.dot} style={{ background: f.color }} />
        <div className={styles.featureBody}>
          <span className={styles.featureText}>{f.text}</span>
          <span className={styles.featureDesc}>{f.desc}</span>
        </div>
      </li>
    ))}
  </ul>
);

const PolkadotLogo = () => (
  <svg className={styles.polkadotLogo} width="20" height="20" viewBox="0 0 256 256" fill="#E6007A" xmlns="http://www.w3.org/2000/svg" class="w-6 h-6 flex-shrink-0" aria-hidden="true">
    <path d="M31.0155 57.7181C14.6547 76.7768 14.2233 103.306 30.0862 116.92C45.9492 130.566 72.0667 126.15 88.4607 107.058C104.821 87.9995 105.253 61.4701 89.3899 47.8567C83.1841 42.511 75.3522 39.9543 67.1884 39.9543C54.5113 39.9543 40.9713 46.1302 31.0155 57.7181Z"></path>
    <path d="M26.2694 156.332C13.9574 170.941 19.3003 195.744 38.2164 211.715C57.1326 227.686 82.4868 228.815 94.7989 214.205C107.111 199.596 101.768 174.793 82.8518 158.822C72.8296 150.355 61.0153 146.072 50.2962 146.072C40.7718 146.072 32.077 149.459 26.3026 156.332"></path>
    <path d="M137.343 209.789C115.142 216.795 99.8429 231.072 103.161 241.664C106.513 252.256 127.221 255.178 149.423 248.139C171.625 241.133 186.923 226.856 183.605 216.264C181.481 209.59 172.454 205.938 160.507 205.938C153.505 205.938 145.54 207.166 137.343 209.756"></path>
    <path d="M102.597 18.5365C98.0176 31.7514 112.553 48.8179 135.12 56.6871C157.686 64.5562 179.689 60.2066 184.268 46.9917C188.848 33.7768 174.313 16.7103 151.746 8.84109C144.146 6.18482 136.58 4.9231 129.744 4.9231C116.303 4.9231 105.617 9.77078 102.597 18.5365Z"></path>
    <path d="M204.048 45.169C197.51 47.7921 199.07 66.884 207.499 87.7357C215.928 108.621 228.041 123.396 234.579 120.773C241.083 118.15 239.557 99.0912 231.128 78.2063C223.362 58.9484 212.444 44.8702 205.674 44.8702C205.11 44.8702 204.579 44.9698 204.048 45.169Z"></path>
    <path d="M209.058 172.038C199.766 192.192 196.547 210.553 201.89 213.01C207.233 215.468 219.114 201.124 228.406 180.969C237.731 160.815 240.917 142.453 235.607 139.996C235.209 139.797 234.778 139.731 234.28 139.731C228.472 139.731 217.654 153.411 209.058 172.038Z"></path>
  </svg>
);



const CtaCardContent = () => (
  <>
    <div className={styles.ctaTop}>
      <span className={styles.ctaEyebrow}>Get started</span>
      <p className={styles.ctaHeadline}>
        Ready to build with <em className={styles.ctaEm}>Pallet-XP ?</em>
      </p>
      <p className={styles.ctaDesc}>
        Pre-configured template & runtime, example pallet, and full documentation included.
      </p>
    </div>
    <div className={styles.ctaHr} />
    <div className={styles.ctaBody}>
      <div className={styles.ctaSteps}>
        <div className={styles.ctaStep}><span className={styles.ctaNum}>01</span>Clone the <a className={styles.inlinelink} href="https://github.com/auguth/xp-substrate-template" target="_blank">template repository</a></div>
        <div className={styles.ctaStep}><span className={styles.ctaNum}>02</span>Follow the <Link className={styles.inlinelink} to="/docs/getting-started/installation">Installation Guide</Link></div>
        <div className={styles.ctaStep}><span className={styles.ctaNum}>03</span>⁠Build your own XP powered Runtime</div>
      </div>
      {/* <div className={styles.ctaBtns}>
        <a href="https://github.com/auguth/xp-substrate-template"
          className={styles.btnPrimary}
          target="_blank" rel="noopener noreferrer">
          Start with Template
        </a>
        <Link
          to="/docs/intro"
          className={styles.btnSecondary}>
          Read the docs
        </Link>
      </div> */}
    </div>
    <div className={styles.ctaHr} />
    <div className={styles.sdkBadges}>
      <a href="https://github.com/paritytech/polkadot-sdk" className={styles.sdkBadge} target="_blank" rel="noopener noreferrer">
        <PolkadotLogo />Powered by Polkadot SDK
      </a>
      <a href="https://docs.substrate.io" className={styles.subBadge} target="_blank" rel="noopener noreferrer">
        Built with Substrate & FRAME
      </a>
    </div>
  </>
);

const CtaCardDesktop = () => (
  <div className={styles.ctaCard}>
    <CtaCardContent />
  </div>
);

const CtaCardTablet = () => (
  <div className={styles.ctaCard}>
    <div className={styles.tabletCtaRow}>
      <div className={styles.tabletCtaContent}>
        <CtaCardContent />
      </div>
      <div className={styles.tabletCtaImage}>
        <img src={mascotImg} alt="Pallet XP mascot" className={styles.mascotImg} />
      </div>
    </div>
  </div>
);

export default function CompSec8() {
  return (
    <section className={styles.section}>
      <div className={styles.inner}>

        <div className={styles.titleRow}>
          <div className={styles.titleBlock}>
            <span className={styles.eyebrow}>For builders</span>
            <h2 className={styles.title}>
              Built for Developers.
              <span className={styles.titleAccent}> Loved by Builders.</span>
            </h2>
          </div>
        </div>

        <div className={styles.divider} />

        <div className={styles.grid}>
          <FeatureList features={leftFeatures} />
          <div className={styles.colRule} />
          <div className={styles.imageCol}>
            <CtaCardDesktop />
          </div>
          <div className={styles.colRule} />
          <FeatureList features={rightFeatures} />
        </div>

        <div className={styles.tabletBlock}>
          <div className={styles.titleBlock}>
            <span className={styles.eyebrow}>For builders</span>
            <h2 className={styles.title}>
              Built for Developers.<br />
              <span className={styles.titleAccent}>Loved by Builders.</span>
            </h2>
          </div>

          <div className={styles.divider} />

          <div className={styles.tabletGrid}>
            <FeatureList features={leftFeatures} />
            <div className={styles.colRule} />
            <FeatureList features={rightFeatures} />
          </div>

          <div className={styles.divider} />

          <CtaCardTablet />
        </div>

        <div className={styles.mobileBlock}>
          <div className={styles.titleBlock}>
            <span className={styles.eyebrow}>For builders</span>
            <h2 className={styles.title}>
              Built for Developers.<br />
              <span className={styles.titleAccent}>Loved by Builders.</span>
            </h2>
          </div>

          <div className={styles.divider} />

          <FeatureList features={leftFeatures} />

          <div className={styles.divider} />

          <div className={styles.ctaCard}>
            <CtaCardContent />
          </div>

          <div className={styles.divider} />

          <FeatureList features={rightFeatures} />
        </div>

      </div>
    </section>
  );
}