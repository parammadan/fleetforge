// Definitions for the terms an executive should not have to already know.
//
// The rule this enforces: no acronym appears in executive-facing text without a
// definition attached to it. "PDB" is four characters that decide whether a
// fleet can be updated, and a leadership audience that nods past it cannot
// evaluate anything else on the screen.
//
// Definitions are plain English first and precise second — a person who knows
// Kubernetes should not find them wrong, and a person who does not should not
// need a second lookup.

export const GLOSSARY: Record<string, { term: string; definition: string }> = {
  pdb: {
    term: "PodDisruptionBudget",
    definition:
      "A rule the application owner sets saying how many of its copies may be taken offline " +
      "deliberately at one time. Kubernetes enforces it: if the budget says none, the platform " +
      "refuses to remove a pod even for routine maintenance. It protects availability, and it " +
      "is also what can stop an update dead.",
  },
  cordon: {
    term: "cordon",
    definition:
      "Marking a machine as unavailable for new work. Pods already running stay put; nothing " +
      "new is placed there. It is the first step of taking a node out of service, and a cordon " +
      "left in place is a machine quietly removed from your capacity.",
  },
  eviction: {
    term: "eviction",
    definition:
      "The polite way to remove a running pod: the platform asks, the PodDisruptionBudget gets " +
      "a veto, and the application shuts down cleanly. Distinct from a pod being killed — a " +
      "kill respects no budget.",
  },
  drain: {
    term: "drain",
    definition:
      "Cordoning a machine and then evicting everything on it, so the machine can be rebooted " +
      "or replaced safely. A drain that cannot finish leaves the cordon behind.",
  },
  brupop: {
    term: "Brupop",
    definition:
      "The Bottlerocket Update Operator — AWS software that updates Bottlerocket machines one " +
      "at a time: cordon, drain, update, reboot, uncordon. It performed this update. FleetForge " +
      "only watched.",
  },
  bottlerocket: {
    term: "Bottlerocket",
    definition:
      "A minimal AWS operating system built only for running containers. It has no package " +
      "manager and is updated as a whole image, which is why an update means a reboot.",
  },
  pending: {
    term: "Pending",
    definition:
      "A pod that Kubernetes has accepted but has not been able to place on any machine. It is " +
      "not starting slowly; it has nowhere to start.",
  },
  snapshot: {
    term: "snapshot hash",
    definition:
      "A SHA-256 fingerprint of everything FleetForge observed at one moment, computed over a " +
      "canonical form so the same cluster state always produces the same hash. Two results " +
      "carrying the same hash were computed against identical input.",
  },
  preflight: {
    term: "preflight",
    definition:
      "FleetForge's read-only analysis of what would happen if a node were taken out of " +
      "service now. It never acts; the answer is always a prediction, never a record.",
  },
  cni: {
    term: "CNI",
    definition:
      "Container Network Interface — the plugin that gives each pod an IP address and connects " +
      "it to the rest of the cluster. On EKS that is the VPC CNI, and if it is installed after " +
      "machines have already joined, pod-to-pod networking can be broken in ways that look " +
      "intermittent.",
  },
  resourceVersion: {
    term: "resourceVersion",
    definition:
      "Kubernetes' own version counter for an object. Quoting it pins an observation to an " +
      "exact revision, so a reader can tell whether two claims were made about the same state.",
  },
};

/**
 * A term with its definition available on hover, focus, and to a screen reader.
 *
 * `<abbr>` is the semantically correct element and browsers already give it
 * tooltip behaviour; the dotted underline is added back because the default
 * styling is close to invisible. `tabIndex` makes it reachable by keyboard,
 * which the native element is not.
 */
export function Term({ k, children }: { k: keyof typeof GLOSSARY; children?: React.ReactNode }) {
  const entry = GLOSSARY[k];
  if (!entry) return <>{children}</>;
  return (
    <abbr className="term" title={`${entry.term}: ${entry.definition}`} tabIndex={0}>
      {children ?? entry.term}
    </abbr>
  );
}

/** The full list, rendered once so the definitions exist somewhere permanent. */
export function GlossaryList() {
  return (
    <dl className="glossary">
      {Object.entries(GLOSSARY).map(([key, entry]) => (
        <div key={key}>
          <dt>{entry.term}</dt>
          <dd>{entry.definition}</dd>
        </div>
      ))}
    </dl>
  );
}
