// causm-kernel/src/proof/Proof.Causm.Lattice.fst
module Proof.Causm.Lattice

open Spec.Causm.Lattice

val lemma_consumed_not_readable : #a:Type0 -> st:entropic_state a -> clk:nat ->
  Lemma (requires tag_of st == TagConsumed)
        (ensures is_readable st clk == false)
let lemma_consumed_not_readable #a st clk = ()

val lemma_decayed_not_readable : #a:Type0 -> st:entropic_state a -> clk:nat ->
  Lemma (requires tag_of st == TagDecayed)
        (ensures is_readable st clk == false)
let lemma_decayed_not_readable #a st clk = ()

val lemma_consume_only_valid : #a:Type0 -> st:entropic_state a ->
  Lemma (requires is_consumable st == true)
        (ensures tag_of st == TagValid)
let lemma_consume_only_valid #a st = ()
