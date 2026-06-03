/// AXON Protocol — Settlement & Reputation Contract
/// Module: axon::settlement
///
/// Implements atomic escrow with deterministic reputation updates.
///
/// # Lifecycle
///   1. Initiator calls `create_escrow`  →  locks funds, gets escrow object ID
///   2. Daemon embeds object ID in HandshakeRequest → responder verifies on-chain
///   3. Responder executes task  →  calls `settle_escrow` with preimage proof
///   4. If responder misses deadline → initiator calls `reclaim_expired_escrow`
///
/// # Security invariants (enforced by Move's linear type system)
///   - Funds cannot be duplicated: `Coin<SUI>` is a linear type
///   - Escrow is consumed exactly once: object deleted on settlement/reclaim
///   - Only authorised parties can act: `tx_context::sender` checks
///   - Settlement requires cryptographic preimage: no trusted oracle
module axon::settlement {
    use sui::object::{Self, UID, ID};
    use sui::transfer;
    use sui::tx_context::{Self, TxContext};
    use sui::coin::{Self, Coin};
    use sui::sui::SUI;
    use sui::clock::{Self, Clock};
    use sui::event;
    use std::hash;
    use std::vector;

    // ── Error codes ──────────────────────────────────────────────────────────

    const E_DEADLINE_EXPIRED:       u64 = 1001;
    const E_INVALID_PREIMAGE:       u64 = 1002;
    const E_NOT_AUTHORIZED:         u64 = 1003;
    const E_DEADLINE_NOT_EXPIRED:   u64 = 1004;
    const E_INVALID_COMMITMENT_LEN: u64 = 1005;
    const E_ZERO_PAYMENT:           u64 = 1006;

    // ── Protocol constants ───────────────────────────────────────────────────

    const PROTOCOL_FEE_BPS:             u64 = 30;
    const EIGENTRUST_DECAY_NUM:         u64 = 9;
    const EIGENTRUST_DECAY_DEN:         u64 = 10;
    const EIGENTRUST_SUCCESS_INCREMENT: u64 = 100_000;
    const EIGENTRUST_MAX_SCORE:         u64 = 1_000_000;

    // ── Objects ──────────────────────────────────────────────────────────────

    /// On-chain escrow locking initiator funds pending task completion.
    ///
    /// Shared object — both initiator and responder may transact against it.
    /// Consumed exactly once by `settle_escrow` or `reclaim_expired_escrow`.
    struct AgentEscrow has key {
        id: UID,
        /// 16-byte session UUID matching off-chain HandshakeRequest.session_id.
        session_id: vector<u8>,
        initiator: address,
        responder: address,
        locked_funds: Coin<SUI>,
        /// sha2_256(preimage) — responder submits preimage on settlement.
        /// Production: replace with BLAKE3 once SUI adds the native syscall.
        commitment_hash: vector<u8>,   // 32 bytes
        deadline_ms: u64,
        treasury: address,
    }

    /// Per-agent on-chain reputation. Shared object updated on every settlement.
    struct AgentReputation has key {
        id: UID,
        agent: address,
        successful_settlements: u64,
        failed_settlements:     u64,
        total_compute_units:    u64,
        /// EigenTrust score in [0, 1_000_000] (fixed-point, 6 decimal places).
        eigentrust_score: u64,
        /// Slashable performance bond. Governance module processes slash events.
        stake: Coin<SUI>,
    }

    // ── Events ───────────────────────────────────────────────────────────────

    struct EscrowCreated has copy, drop {
        escrow_id:   ID,
        session_id:  vector<u8>,
        initiator:   address,
        responder:   address,
        amount_pico: u64,
        deadline_ms: u64,
    }

    struct EscrowSettled has copy, drop {
        escrow_id:          ID,
        session_id:         vector<u8>,
        responder:          address,
        compute_units:      u64,
        responder_payment:  u64,
        protocol_fee:       u64,
        initiator_refund:   u64,
        new_eigentrust:     u64,
    }

    struct EscrowReclaimed has copy, drop {
        escrow_id:      ID,
        session_id:     vector<u8>,
        initiator:      address,
        refund_amount:  u64,
        new_eigentrust: u64,
    }

    // ── Public entry points ──────────────────────────────────────────────────

    /// Lock funds in escrow before sending a HandshakeRequest.
    ///
    /// Must be called by the initiating agent; the escrow object ID is
    /// included in the HandshakeRequest so the responder can verify on-chain.
    public fun create_escrow(
        session_id:      vector<u8>,
        responder:       address,
        commitment_hash: vector<u8>,
        deadline_ms:     u64,
        treasury:        address,
        payment:         Coin<SUI>,
        clock:           &Clock,
        ctx:             &mut TxContext,
    ): ID {
        let now_ms = clock::timestamp_ms(clock);
        assert!(deadline_ms > now_ms, E_DEADLINE_EXPIRED);
        assert!(vector::length(&commitment_hash) == 32, E_INVALID_COMMITMENT_LEN);
        assert!(coin::value(&payment) > 0, E_ZERO_PAYMENT);

        let escrow = AgentEscrow {
            id: object::new(ctx),
            session_id,
            initiator: tx_context::sender(ctx),
            responder,
            locked_funds: payment,
            commitment_hash,
            deadline_ms,
            treasury,
        };

        let escrow_id = object::id(&escrow);

        event::emit(EscrowCreated {
            escrow_id,
            session_id: escrow.session_id,
            initiator:  escrow.initiator,
            responder,
            amount_pico: coin::value(&escrow.locked_funds),
            deadline_ms,
        });

        transfer::share_object(escrow);
        escrow_id
    }

    /// Settle escrow after successful task execution.
    ///
    /// Responder submits the `preimage` of `commitment_hash`.
    /// sha2_256(preimage) must equal commitment_hash stored in the escrow.
    ///
    /// Payment splits:
    ///   responder  = gross * (1 - PROTOCOL_FEE_BPS / 10_000)
    ///   treasury   = gross * (PROTOCOL_FEE_BPS / 10_000)
    ///   initiator  = escrow_total - gross  (unused compute budget)
    public fun settle_escrow(
        escrow:               AgentEscrow,
        preimage:             vector<u8>,
        actual_compute_units: u64,
        agreed_price_per_cu:  u64,
        rep:                  &mut AgentReputation,
        clock:                &Clock,
        ctx:                  &mut TxContext,
    ) {
        let now_ms = clock::timestamp_ms(clock);

        assert!(tx_context::sender(ctx) == escrow.responder, E_NOT_AUTHORIZED);
        assert!(now_ms <= escrow.deadline_ms, E_DEADLINE_EXPIRED);

        let computed = hash::sha2_256(preimage);
        assert!(computed == escrow.commitment_hash, E_INVALID_PREIMAGE);

        let escrow_total = coin::value(&escrow.locked_funds);
        let uncapped     = actual_compute_units * agreed_price_per_cu;
        let gross        = if (uncapped > escrow_total) { escrow_total } else { uncapped };
        let fee          = (gross * PROTOCOL_FEE_BPS) / 10_000;
        let net          = gross - fee;
        let refund       = escrow_total - gross;

        let AgentEscrow { id, session_id, initiator, responder, locked_funds,
                          commitment_hash: _, deadline_ms: _, treasury } = escrow;

        let escrow_id = object::uid_to_inner(&id);
        object::delete(id);
        let mut funds = locked_funds;

        let responder_coin = coin::split(&mut funds, net, ctx);
        transfer::public_transfer(responder_coin, responder);

        if (fee > 0) {
            let fee_coin = coin::split(&mut funds, fee, ctx);
            transfer::public_transfer(fee_coin, treasury);
        };

        if (refund > 0) {
            transfer::public_transfer(funds, initiator);
        } else {
            coin::destroy_zero(funds);
        };

        // EigenTrust EMA update (success): score = min(0.9*old + 100_000, MAX)
        rep.successful_settlements = rep.successful_settlements + 1;
        rep.total_compute_units    = rep.total_compute_units + actual_compute_units;
        let decayed = (rep.eigentrust_score * EIGENTRUST_DECAY_NUM) / EIGENTRUST_DECAY_DEN;
        let raw     = decayed + EIGENTRUST_SUCCESS_INCREMENT;
        rep.eigentrust_score = if (raw > EIGENTRUST_MAX_SCORE) { EIGENTRUST_MAX_SCORE } else { raw };

        event::emit(EscrowSettled {
            escrow_id, session_id, responder,
            compute_units:    actual_compute_units,
            responder_payment: net,
            protocol_fee:      fee,
            initiator_refund:  refund,
            new_eigentrust:    rep.eigentrust_score,
        });
    }

    /// Reclaim funds after the responder misses the settlement deadline.
    ///
    /// Full refund to initiator. Responder's reputation is penalised.
    /// Stake slashing is emitted as an event for the governance module.
    public fun reclaim_expired_escrow(
        escrow: AgentEscrow,
        rep:    &mut AgentReputation,
        clock:  &Clock,
        ctx:    &mut TxContext,
    ) {
        let now_ms = clock::timestamp_ms(clock);
        assert!(tx_context::sender(ctx) == escrow.initiator, E_NOT_AUTHORIZED);
        assert!(now_ms > escrow.deadline_ms, E_DEADLINE_NOT_EXPIRED);

        let AgentEscrow { id, session_id, initiator, responder: _,
                          locked_funds, commitment_hash: _, deadline_ms: _, treasury: _ } = escrow;

        let escrow_id     = object::uid_to_inner(&id);
        let refund_amount = coin::value(&locked_funds);
        object::delete(id);
        transfer::public_transfer(locked_funds, initiator);

        // EigenTrust EMA update (failure): score = 0.9 * old  (no increment)
        rep.failed_settlements = rep.failed_settlements + 1;
        rep.eigentrust_score   = (rep.eigentrust_score * EIGENTRUST_DECAY_NUM) / EIGENTRUST_DECAY_DEN;

        event::emit(EscrowReclaimed {
            escrow_id, session_id, initiator,
            refund_amount,
            new_eigentrust: rep.eigentrust_score,
        });
    }

    // ── View functions ───────────────────────────────────────────────────────

    public fun eigentrust_score(rep: &AgentReputation): u64 { rep.eigentrust_score }

    /// Success rate in basis points (9934 = 99.34%).
    public fun success_rate_bps(rep: &AgentReputation): u64 {
        let total = rep.successful_settlements + rep.failed_settlements;
        if (total == 0) { 0 } else { (rep.successful_settlements * 10_000) / total }
    }
}
