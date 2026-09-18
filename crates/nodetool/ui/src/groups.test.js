// The group composition, client-side: the identity derivation mirrored
// from the compiler's (pinned against its values), the facts a document's
// groups compose — boundary emissions, inner identities, ownership, the
// status aggregate — the collapsed node's rendering by the default class
// with the group's ports, and the re-keying of the run display's values so
// an inside pulse stays inside and a boundary emission shows at the port
// it crosses.
import { describe, expect, it } from 'vitest'
import { canvasValues, groupFacts, innerIdentity, instanceStatus } from './groups.js'
import { nodeMarks, toEdges, toNodes } from './view.js'

const INNER = '3f2b8a1c-6d54-4e8a-9b7e-1c2d3e4f5a6b'
const INSTANCE = '00000000-0000-0000-0000-600000000002'
const OUTER = '00000000-0000-0000-0000-600000000003'
const MID = '00000000-0000-0000-0000-600000000004'
const DEEP = '00000000-0000-0000-0000-600000000005'
const LOOSE = '00000000-0000-0000-0000-600000000006'
const PLAIN = '00000000-0000-0000-0000-600000000007'
const SOURCE = '00000000-0000-0000-0000-600000000008'

describe('innerIdentity', () => {
  it('mirrors the compiler derivation, pinned to its values', () => {
    expect(innerIdentity(['00000000-0000-0000-0000-0000000000e1'], '00000000-0000-0000-0000-0000000000f1')).toBe(
      '00000000-0000-0000-0000-000000000020',
    )
    expect(innerIdentity([], INNER)).toBe(INNER)
    expect(innerIdentity([INSTANCE], INNER)).toBe('7e571438-daa8-9d15-36fc-f85a7c9eb4d2')
    expect(innerIdentity([INSTANCE, OUTER], INNER)).toBe('fcae2871-b551-3a2a-6df9-30b4f93d69a2')
    expect(
      innerIdentity(
        ['00000000-0000-0000-0000-0000000000e2', '00000000-0000-0000-0000-0000000000e1'],
        '00000000-0000-0000-0000-0000000000f1',
      ),
    ).toBe('00000000-0000-0000-0000-00000000018e')
  })

  it('derives distinct identities for the same node under different chains', () => {
    expect(innerIdentity([INSTANCE], INNER)).not.toBe(innerIdentity([OUTER], INNER))
    expect(innerIdentity([INSTANCE, OUTER], INNER)).not.toBe(innerIdentity([OUTER, INSTANCE], INNER))
  })
})

// One instance of a group that exposes one output bound to its inner node.
const shoutGraph = {
  nodes: [
    { uuid: SOURCE, type_ref: 'text/words', metadata: {} },
    { uuid: INSTANCE, type_ref: 'shout', metadata: {} },
    { uuid: PLAIN, type_ref: 'text/uppercase', metadata: {} },
  ],
  edges: [
    { from: SOURCE, from_port: 'out', to: INSTANCE, to_port: 'text' },
    { from: INSTANCE, from_port: 'text', to: PLAIN, to_port: 'text' },
  ],
  groups: [
    {
      name: 'shout',
      inputs: [{ name: 'text', type_refs: ['String'], node: INNER, port: 'text' }],
      outputs: [{ name: 'text', type_refs: ['String'], node: INNER, port: 'text' }],
      nodes: [
        { uuid: INNER, type_ref: 'text/uppercase' },
        { uuid: LOOSE, type_ref: 'text/uppercase' },
      ],
      edges: [],
    },
  ],
}

const shoutIdentity = innerIdentity([INSTANCE], INNER)
const insideIdentity = innerIdentity([INSTANCE], LOOSE)

describe('groupFacts', () => {
  it('is null for a document without groups', () => {
    expect(groupFacts({ nodes: [], edges: [] })).toBeNull()
  })

  it('maps a boundary emission to the instance port that shows it', () => {
    const facts = groupFacts(shoutGraph)
    expect(facts.emissions.get(`${shoutIdentity}/text`)).toEqual({
      instance: INSTANCE,
      port: 'text',
    })
  })

  it('holds every inner identity, its owner, and the inside of the instance', () => {
    const facts = groupFacts(shoutGraph)
    expect(facts.inner.has(shoutIdentity)).toBe(true)
    expect(facts.owner.get(shoutIdentity)).toBe(INSTANCE)
    expect(facts.inside.get(INSTANCE)).toEqual([shoutIdentity, insideIdentity])
  })

  it('follows an exposed output through a nested instance to the node that emits', () => {
    const facts = groupFacts({
      nodes: [{ uuid: OUTER, type_ref: 'outer-stage', metadata: {} }],
      edges: [],
      groups: [
        {
          name: 'outer-stage',
          inputs: [],
          outputs: [{ name: 'out', type_refs: ['String'], node: MID, port: 'text' }],
          nodes: [
            { uuid: MID, type_ref: 'inner-stage' },
            { uuid: LOOSE, type_ref: 'text/uppercase' },
          ],
          edges: [],
        },
        {
          name: 'inner-stage',
          inputs: [],
          outputs: [{ name: 'text', type_refs: ['String'], node: DEEP, port: 'text' }],
          nodes: [{ uuid: DEEP, type_ref: 'text/uppercase' }],
          edges: [],
        },
      ],
    })
    const deep = innerIdentity([OUTER, MID], DEEP)
    expect(facts.emissions.get(`${deep}/text`)).toEqual({ instance: OUTER, port: 'out' })
    // An inside emission with no boundary shows nothing.
    const loose = innerIdentity([OUTER], LOOSE)
    expect(facts.emissions.has(`${loose}/text`)).toBe(false)
    expect(facts.inside.get(OUTER)).toEqual([
      innerIdentity([OUTER], MID),
      deep,
      loose,
    ])
  })
})

describe('instanceStatus', () => {
  const statusesOf = (inner, states) =>
    Object.fromEntries(inner.map((uuid, index) => [uuid, states[index]]))

  it('is nothing while no inner node has started', () => {
    expect(instanceStatus({}, [shoutIdentity])).toBeUndefined()
    expect(instanceStatus(statusesOf([], []), [])).toBeUndefined()
  })

  it('is completed only when every inner node is', () => {
    expect(
      instanceStatus(statusesOf([shoutIdentity, insideIdentity], ['completed', 'completed']), [
        shoutIdentity,
        insideIdentity,
      ]),
    ).toBe('completed')
    expect(
      instanceStatus(statusesOf([shoutIdentity], ['completed']), [shoutIdentity, insideIdentity]),
    ).toBeUndefined()
  })

  it('an inner error is the collapsed node error', () => {
    expect(
      instanceStatus(statusesOf([shoutIdentity, insideIdentity], ['completed', 'failed']), [
        shoutIdentity,
        insideIdentity,
      ]),
    ).toBe('failed')
  })

  it('the closure of the inside reads as stopped', () => {
    expect(
      instanceStatus(statusesOf([shoutIdentity, insideIdentity], ['stopped', 'completed']), [
        shoutIdentity,
        insideIdentity,
      ]),
    ).toBe('stopped')
  })

  it('runs only while inner nodes run', () => {
    expect(
      instanceStatus(statusesOf([shoutIdentity, insideIdentity], ['running', 'completed']), [
        shoutIdentity,
        insideIdentity,
      ]),
    ).toBe('running')
  })
})

describe('canvasValues', () => {
  it('re-keys a boundary emission to the instance port and drops what stays inside', () => {
    const facts = groupFacts(shoutGraph)
    expect(
      canvasValues(
        {
          [`${shoutIdentity}/text`]: 'HELLO',
          [`${insideIdentity}/out`]: 'inside',
          [`${PLAIN}/text`]: 'kept',
        },
        facts,
      ),
    ).toEqual({ [`${INSTANCE}/text`]: 'HELLO', [`${PLAIN}/text`]: 'kept' })
  })

  it('carries the values of a flat document untouched', () => {
    const values = { [`${PLAIN}/text`]: 'kept' }
    expect(canvasValues(values, null)).toEqual(values)
  })
})

const listing = {
  types: [
    {
      type_ref: 'text/words',
      label: 'Words',
      icon: '<svg/>',
      inputs: [],
      outputs: [{ name: 'out', type_refs: ['String'] }],
    },
    {
      type_ref: 'text/uppercase',
      label: 'Uppercase',
      icon: '<svg/>',
      inputs: [{ name: 'text', type_refs: ['String'] }],
      outputs: [{ name: 'text', type_refs: ['String'] }],
    },
  ],
  baseScalars: { String: true },
  dataTypes: { String: { color: '#2d72d2', shape: 'circle' } },
}

describe('toNodes', () => {
  it('renders a group instance by the default class with the group ports, labelled by the group', () => {
    const nodes = toNodes(shoutGraph, listing, undefined, undefined, undefined)
    const instance = nodes.find((node) => node.id === INSTANCE)
    expect(instance.type).toBe('type')
    expect(instance.data.type).toMatchObject({
      type_ref: 'shout',
      label: 'shout',
      icon: null,
      inputs: [{ name: 'text', type_refs: ['String'] }],
      outputs: [{ name: 'text', type_refs: ['String'] }],
    })
    expect(instance.data.scalarInputs).toEqual(['text'])
  })

  it('the status of a group instance is the aggregate of its inside', () => {
    const statuses = { [shoutIdentity]: 'completed', [insideIdentity]: 'completed' }
    const nodes = toNodes(shoutGraph, listing, undefined, statuses, undefined)
    expect(nodes.find((node) => node.id === INSTANCE).data.status).toBe('completed')
    const failed = toNodes(shoutGraph, listing, undefined, { [shoutIdentity]: 'failed' }, undefined)
    expect(failed.find((node) => node.id === INSTANCE).data.status).toBe('failed')
    expect(
      toNodes(shoutGraph, listing, undefined, undefined, undefined).find(
        (node) => node.id === INSTANCE,
      ).data.status,
    ).toBeUndefined()
  })

  it('shows the value of a boundary emission at the instance port', () => {
    const values = canvasValues({ [`${shoutIdentity}/text`]: 'HELLO' }, groupFacts(shoutGraph))
    const nodes = toNodes(shoutGraph, listing, undefined, undefined, values)
    expect(nodes.find((node) => node.id === INSTANCE).data.portValues).toEqual({ text: 'HELLO' })
  })

  it('resolves the type reference of a group before the listing', () => {
    const withListingEntry = {
      ...listing,
      types: [...listing.types, { type_ref: 'shout', label: 'shadowed', inputs: [], outputs: [] }],
    }
    const nodes = toNodes(shoutGraph, withListingEntry, undefined, undefined, undefined)
    expect(nodes.find((node) => node.id === INSTANCE).data.type.label).toBe('shout')
  })

  it('a type reference that is neither a group nor a listed type stays a placeholder', () => {
    const unknown = {
      nodes: [{ uuid: SOURCE, type_ref: 'no/such', metadata: {} }],
      edges: [],
    }
    const nodes = toNodes(unknown, listing, undefined, undefined, undefined)
    expect(nodes[0].type).toBe('placeholder')
  })
})

describe('toEdges', () => {
  it('colours a boundary wire from the exposed port of the group', () => {
    const edges = toEdges(shoutGraph, undefined, listing)
    const boundary = edges.find((edge) => edge.source === INSTANCE)
    expect(boundary.style).toEqual({ stroke: '#2d72d2' })
    const upstream = edges.find((edge) => edge.target === INSTANCE)
    expect(upstream.style).toEqual({ stroke: '#2d72d2' })
  })
})

describe('nodeMarks', () => {
  it('the mark of a failed run lands on the collapsed node the inner failure sits in', () => {
    const marks = nodeMarks(
      [],
      { outcome: 'failed', error: 'the inner node failed', node: shoutIdentity },
      groupFacts(shoutGraph).owner,
    )
    expect(marks.get(INSTANCE)).toEqual(['the inner node failed'])
  })

  it('without groups the mark lands on the node the outcome names', () => {
    const marks = nodeMarks([], { outcome: 'failed', error: 'e', node: PLAIN }, null)
    expect(marks.get(PLAIN)).toEqual(['e'])
  })
})
