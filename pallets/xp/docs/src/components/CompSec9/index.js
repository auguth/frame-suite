import React from 'react';
import styles from './styles.module.css';

const toIcon = () => (
    <svg  xmlns="http://www.w3.org/2000/svg" width="24" height="24"  
        fill="currentColor" viewBox="0 0 24 24" >
        <path d="M17 16V7H8v2h5.59l-6.3 6.29 1.42 1.42 6.29-6.3V16z"></path>
    </svg>
)

const CHANNELS = [
  {
    id: 'issues',
    iconClass: 'iconGh',
    title: 'GitHub Issues',
    desc: 'Found a bug or have a feature request? Open an issue and we\'ll take a look.',
    tag: 'Report a bug',
    href: 'https://github.com/auguth/frame-suite/issues',
    icon: (
      <svg viewBox="0 0 24 24" fill="currentColor">
        <path d="M12 2C6.477 2 2 6.484 2 12.017c0 4.425 2.865 8.18 6.839 9.504.5.092.682-.217.682-.483 
        0-.237-.008-.868-.013-1.703-2.782.605-3.369-1.343-3.369-1.343-.454-1.158-1.11-1.466-1.11-1.466-.908-.62.069-.608.069-.608 
        1.003.07 1.531 1.032 1.531 1.032.892 1.53 2.341 1.088 2.91.832.092-.647.35-1.088.636-1.338-2.22-.253-4.555-1.113-4.555-4.951 
        0-1.093.39-1.988 1.029-2.688-.103-.253-.446-1.272.098-2.65 0 0 .84-.27 2.75 1.026A9.564 9.564 0 0 1 12 6.844a9.59 9.59 0 0 1 
        2.504.337c1.909-1.296 2.747-1.027 2.747-1.027.546 1.379.202 2.398.1 2.651.64.7 1.028 1.595 1.028 2.688 0 3.848-2.339 4.695-4.566 
        4.943.359.309.678.92.678 1.855 0 1.338-.012 2.419-.012 2.747 0 .268.18.58.688.482A10.02 10.02 0 0 0 22 12.017C22 6.484 17.522 2 12 2z"/>
      </svg>
    ),
  },
  {
    id: 'discussions',
    iconClass: 'iconDi',
    title: 'GitHub Discussions',
    desc: 'General questions, ideas, and RFC proposals. Start or join a conversation.',
    tag: 'Join discussion',
    href: 'https://github.com/auguth/frame-suite/discussions',
    icon: (
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
        <path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/>
      </svg>
    ),
  },
  {
    id: 'stackexchange',
    iconClass: 'iconSu',
    title: 'Substrate Stack Exchange',
    desc: 'Technical Q&A for Substrate and FRAME developers. Best for specific how-to questions.',
    tag: 'Ask a question',
    href: 'https://substrate.stackexchange.com',
    icon: (
      <svg aria-hidden="true" class="fc-black-200 d:fc-black-350 mtn1 svg-icon iconGlyphNew"  viewBox="0 0 26 31">
        <path fill="currentColor" d="m22.18 16.32.05.02-.02.01za19 19 0 0 0-2.45 4.14l-.03.07q-.94 2.19-1.29 
        4.6v.03a18 18 0 0 0-.05 4.85H.01v-4.88h15.94l.1-.6L.69 20.48l1.28-4.7 15.55 4.1.22-.46-13.96-7.96 
        2.47-4.22 14.16 8.07.34-.4L9.15 3.47 12.65 0l11.78 11.64 1.25 1.23q-1.97 1.5-3.5 3.44"></path>
      </svg>
    ),
  },
  {
    id: 'telegram',
    iconClass: 'iconTg',
    title: 'Telegram',
    desc: 'Quick questions and community chat. Join the group and say hi.',
    tag: 'Join group',
    href: 'https://t.me/auguthlabs',
    icon: (
      <svg viewBox="0 0 24 24" fill="currentColor">
        <path d="M11.944 0A12 12 0 0 0 0 12a12 12 0 0 0 12 12 12 12 0 0 0 12-12A12 12 0 0 0 12 0a12 12 0 0 0-.056 
        0zm4.962 7.224c.1-.002.321.023.465.14a.506.506 0 0 1 .171.325c.016.093.036.306.02.472-.18 1.898-.962 6.502-1.36 
        8.627-.168.9-.499 1.201-.82 1.23-.696.065-1.225-.46-1.9-.902-1.056-.693-1.653-1.124-2.678-1.8-1.185-.78-.417-1.21.258-1.91.177-.184 3.247-2.977 
        3.307-3.23.007-.032.014-.15-.056-.212s-.174-.041-.249-.024c-.106.024-1.793 1.14-5.061 3.345-.48.33-.913.49-1.302.48-.428-.008-1.252-.241-1.865-.44-.752-.245-1.349-.374-1.297-.789.027-.216.325-.437.893-.663 
        3.498-1.524 5.83-2.529 6.998-3.014 3.332-1.386 4.025-1.627 4.476-1.635z"/>
      </svg>
    ),
  },
  {
    id: 'matrix',
    iconClass: 'iconMx',
    title: 'Matrix',
    desc: 'Decentralised chat for the Substrate ecosystem. Find us in the Auguth Labs room.',
    tag: 'Open room',
    href: 'https://matrix.to/#/#auguthlabs:matrix.org',
    icon: (
      <svg viewBox="0 0 24 24" fill="currentColor">
        <path d="M.632.55v22.9H2.28V24H0V0h2.28v.55zm7.043 7.26v1.157h.033c.309-.443.683-.784 1.117-1.024.433-.245.936-.365 1.5-.365.54 0 1.033.107 1.481.314.448.208.785.582 1.02 1.108.254-.374.6-.706 1.034-.992.434-.287.95-.43 1.546-.43.453 0 .872.056 1.26.167.388.11.716.286.993.53.276.245.489.559.646.951.152.392.23.863.23 1.417v5.728h-2.349V11.52c0-.286-.01-.559-.032-.812a1.755 1.755 0 0 0-.166-.633 1.025 1.025 0 0 0-.386-.419c-.174-.1-.4-.155-.689-.155-.289 0-.53.058-.723.172a1.265 1.265 0 0 0-.46.454 1.932 1.932 0 0 0-.247.626 3.608 3.608 0 0 0-.073.705v4.745h-2.35v-4.656c0-.252-.008-.5-.024-.744a1.991 1.991 0 0 0-.136-.652 1.016 1.016 0 0 0-.383-.457c-.177-.112-.426-.17-.746-.17-.109 0-.248.023-.417.07a1.269 1.269 0 0 0-.476.258 1.435 1.435 0 0 0-.37.518c-.1.22-.154.5-.154.835v5.002H5.312V7.81zm15.693 15.64V.55H21.72V0H24v24h-2.28v-.55z"/>
      </svg>
    ),
  },
  {
    id: 'twitter',
    iconClass: 'iconTw',
    title: 'Twitter / X',
    desc: 'Follow for updates, announcements and ecosystem news.',
    tag: 'Follow us',
    href: 'https://x.com/auguthlabs',
    icon: (
      <svg viewBox="0 0 24 24" fill="currentColor">
        <path d="M18.244 2.25h3.308l-7.227 8.26 8.502 11.24H16.17l-4.714-6.231-5.401 6.231H2.744l7.73-8.835L1.254 2.25H8.08l4.259 5.631 5.905-5.631zm-1.161 17.52h1.833L7.084 4.126H5.117z"/>
      </svg>
    ),
  },
];

export default function CompSec9() {
  return (
    <section className={styles.section}>
      <div className={styles.inner}>

        <div className={styles.titleRow}>
          <span className={styles.eyebrow}>Community</span>
          <h2 className={styles.h1}>
            Need to Reach Us.
            <em className={styles.em}> We're here.</em>
          </h2>
          <p className={styles.sub}>
            Ask questions, report bugs, share ideas. Pick the channel that works best for you.
          </p>
        </div>

        <div className={styles.grid}>
          {CHANNELS.map((c) => (
            <a
              key={c.id}
              href={c.href}
              className={styles.card}
              target="_blank"
              rel="noopener noreferrer"
            >
              <div className={`${styles.icon} ${styles[c.iconClass]}`}>
                {c.icon}
              </div>
              <div className={styles.cardContent}>
                <p className={styles.cardTitle}>{c.title}</p>
                <p className={styles.cardDesc}>{c.desc}</p>
              </div>
              <span className={styles.cardTag}>{c.tag}
                <svg  xmlns="http://www.w3.org/2000/svg" width="15" height="15"   
                fill="currentColor" viewBox="0 0 24 24" >
                <path d="M17 16V7H8v2h5.59l-6.3 6.29 1.42 1.42 6.29-6.3V16z"></path>
                </svg>              
              </span>
            </a>
          ))}

          {/* <div className={`${styles.card} ${styles.cardHint}`}>
            <span className={styles.hintEmoji}>💬</span>
            <div className={styles.cardContent}>
              <p className={styles.cardTitle}>Not sure where to start?</p>
              <p className={styles.cardDesc}>
                Discussions for general questions. Issues for bugs. Stack Exchange for deep technical dives.
              </p>
            </div>
          </div> */}
        </div>

        <div className={styles.mobileList}>
          {CHANNELS.map((c) => (
            <a
              key={c.id}
              href={c.href}
              className={styles.mobileCard}
              target="_blank"
              rel="noopener noreferrer"
            >
              <div className={`${styles.icon} ${styles[c.iconClass]}`}>
                {c.icon}
              </div>
              <div className={styles.mobileCardContent}>
                <p className={styles.cardTitle}>{c.title}</p>
                <p className={styles.mobileCardDesc}>{c.desc}</p>
              </div>
              <span className={styles.mobileArrow}>
                <svg  xmlns="http://www.w3.org/2000/svg" width="15" height="15"   
                  fill="currentColor" viewBox="0 0 24 24" >
                  <path d="M17 16V7H6v2h7.59l-8.3 8.29 1.42 1.42 8.29-8.3V16z"/>
                </svg>
              </span>
            </a>
          ))}
        </div>

      </div>
    </section>
  );
}