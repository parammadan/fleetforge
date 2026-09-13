// The investigation chain.
//
//   two cordoned nodes → replica Pending → currentHealthy=2 → disruptionsAllowed=0 → eviction blocked
//
// Drawn as five boxes and four arrows, and the arrows are drawn differently
// from the boxes on purpose. Every incident-review diagram ever made uses one
// uniform arrow, which quietly grants a causal claim the same standing as a
// measurement. Here the boxes are solid and green or blue — observed, derived.
// The arrows are dashed and purple, tagged HUMAN RCA, and each one states the
// reasoning it is carrying.
//
// A reader who disagrees with the conclusion can point at exactly which arrow
// they disagree with. That is the whole design.

import { useState } from "react";
import type { ChainEdge, ChainLink, InvestigationChain as Chain } from "./replayTypes";
import { BASIS_LABEL, BASIS_MEANING } from "./replayTypes";
import { BasisTag, Panel } from "./ReplayPanels";
import { Term } from "./glossary";

export function InvestigationChainPanel({
  chain,
  onOpenEvidence,
}: {
  chain: Chain;
  onOpenEvidence: (artifact: string) => void;
}) {
  const [open, setOpen] = useState<string | null>(chain.links[0]?.id ?? null);
  const edgeAfter = (id: string): ChainEdge | undefined =>
    chain.edges.find((e) => e.from === id);

  return (
    <Panel
      id="chain"
      title="Investigation chain"
      subtitle={`${chain.links.length} observations · ${chain.edges.length} inferred links`}
    >
      <p className="chain-intro">
        Two <Term k="cordon">cordoned</Term> nodes → a replica stuck{" "}
        <Term k="pending">Pending</Term> → currentHealthy 2 → disruptionsAllowed 0 →{" "}
        <Term k="eviction">eviction</Term> blocked.
      </p>

      <ol className="chain" aria-label="Investigation chain">
        {chain.links.map((link, i) => (
          <li key={link.id}>
            <ChainBox
              link={link}
              expanded={open === link.id}
              onToggle={() => setOpen(open === link.id ? null : link.id)}
              onOpenEvidence={onOpenEvidence}
              step={i + 1}
            />
            {edgeAfter(link.id) && <Arrow edge={edgeAfter(link.id) as ChainEdge} />}
          </li>
        ))}
      </ol>

      <p className="chain-attribution">
        <BasisTag basis="human_rca" /> {chain.attribution}
      </p>
    </Panel>
  );
}

function ChainBox({
  link,
  expanded,
  onToggle,
  onOpenEvidence,
  step,
}: {
  link: ChainLink;
  expanded: boolean;
  onToggle: () => void;
  onOpenEvidence: (artifact: string) => void;
  step: number;
}) {
  return (
    <div className={`chain-box chain-${link.basis}`}>
      <button type="button" className="chain-head" onClick={onToggle} aria-expanded={expanded}>
        <span className="chain-step" aria-hidden="true">
          {step}
        </span>
        <span className="chain-label">{link.label}</span>
        <span className="chain-value mono">{link.value}</span>
        <BasisTag basis={link.basis} />
        <span className="chain-toggle" aria-hidden="true">
          {expanded ? "−" : "+"}
        </span>
      </button>
      {expanded && (
        <div className="chain-detail">
          <p>{link.detail}</p>
          <p className="chain-source">
            Read from <code>{link.field_path}</code> in{" "}
            <button
              type="button"
              className="link-button"
              onClick={() => onOpenEvidence(link.artifact)}
            >
              {link.artifact}
            </button>
            {link.at && <> · observed {link.at.slice(11, 19)}</>}
          </p>
        </div>
      )}
    </div>
  );
}

/**
 * The arrow.
 *
 * Dashed, purple, and carrying its own tag — it is not the same kind of claim
 * as the boxes it joins, and it must not look like one.
 */
function Arrow({ edge }: { edge: ChainEdge }) {
  return (
    <div className="chain-arrow" title={BASIS_MEANING[edge.basis]}>
      <svg width="18" height="34" viewBox="0 0 18 34" aria-hidden="true" focusable="false">
        <line
          x1="9"
          y1="0"
          x2="9"
          y2="24"
          strokeDasharray="4 3"
          strokeWidth="2"
          className="arrow-line"
        />
        <path d="M4 23 L9 32 L14 23 Z" className="arrow-head" />
      </svg>
      <span className="chain-because">
        <span className="arrow-basis">{BASIS_LABEL[edge.basis]}</span> {edge.because}
      </span>
    </div>
  );
}
