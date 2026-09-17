// The wire shapes. Every edge draws as React Flow's default bezier; the
// one exception is a wire a node runs back into itself: between two ports
// of one node the default curve degenerates to a flat line the node
// covers, so that one loops over the node's top instead — the only wire
// that needs the node's box, to clear it.
import { BaseEdge, useInternalNode } from '@xyflow/react'

const LOOP_RISE = 40
const LOOP_SPREAD = 60

export function SelfLoopEdge({
  id,
  source,
  sourceX,
  sourceY,
  targetX,
  targetY,
  markerEnd,
  style,
}) {
  const top = useInternalNode(source)?.internals.positionAbsolute.y ?? sourceY
  const rise = sourceY - top + LOOP_RISE
  const path = [
    `M ${sourceX} ${sourceY}`,
    `C ${sourceX + LOOP_SPREAD} ${top - rise}`,
    `${targetX - LOOP_SPREAD} ${top - rise}`,
    `${targetX} ${targetY}`,
  ].join(' ')
  return <BaseEdge id={id} path={path} markerEnd={markerEnd} style={style} />
}
