# Discovery And Design Playbook

Discovery and design should run together for client-facing features.

## Why

Teams lose time when they treat design as a late-stage polish step. For a portal that replaces Confluence, Jira, BA handoff, and QA visibility, the client has to react early to something tangible.

That means discovery should not end with only text notes. It should end with:

- explicit questions
- explicit acceptance criteria
- explicit design brief
- explicit mockup options
- explicit approval

## Stage Contract

```mermaid
flowchart TD
    A[Planned] --> B[Discovery]
    B --> C{Discovery Ready?}
    C -->|No| B
    C -->|Yes| D[Build]
    D --> E[Test]
    E --> F[Done]

    B --> B1[Research Doc]
    B --> B2[Discovery Doc]
    B --> B3[Design Brief]
    B --> B4[Mockup Options]
    B --> B5[Client Approval]
```

## Discovery Checklist

### Always required

- research or discovery artifact
- blocking questions resolved
- acceptance criteria documented

### Required for client-facing features

- design brief
- quick mockup options
- one approved direction

## What Counts As Client-Facing

The system should assume design is required for features that involve:

- websites
- dashboards
- portals
- onboarding flows
- customer workflows
- branded UX
- frontend experiences

## Mockup Standard

Mockups do not need to be production code. They need to answer:

- what is the first impression?
- what is the information hierarchy?
- what proof builds trust?
- what action should the client or end-user take?

## Approval Standard

Approval should be recorded in the portal and attached to the feature itself.

Minimum record:

- approved option ID
- actor
- timestamp
- optional note

## Parallel Delivery Model

```mermaid
sequenceDiagram
    participant Client
    participant Portal
    participant VDD
    participant BuildA as Branch A
    participant BuildB as Branch B
    participant QA

    Client->>Portal: Request outcome
    Portal->>VDD: Create discovery feature
    Portal->>VDD: Record questions and design brief
    Portal->>VDD: Seed mockup options
    Client->>Portal: Approve one option
    Portal->>VDD: Record approval
    VDD->>BuildA: Start implementation lane 1
    VDD->>BuildB: Start implementation lane 2
    BuildA->>VDD: Commit and task updates
    BuildB->>VDD: Commit and task updates
    QA->>VDD: Test evidence and acceptance verification
    VDD->>Portal: Update client-facing progress
```

## Human Explanation

The portal should always be able to explain:

1. what problem is being solved
2. what direction was approved
3. what is being built now
4. what is under test
5. what remains before release

If the portal cannot explain those five things, the workflow is incomplete.
