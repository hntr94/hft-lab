//! Messages that travel between the bot and the exchange.

use crate::ids::OrderId;
use crate::units::{Cash, Price, Qty, Side};

/// How an order wants to trade.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OrderType {
    /// Trade at `price` or better; any unfilled part rests in the book.
    Limit {
        /// The worst price you accept.
        price: Price,
    },
    /// Trade immediately at whatever prices are available; never rests.
    Market,
}

/// A request to place a new order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewOrder {
    /// Unique id chosen by the sender.
    pub id: OrderId,
    /// Buy or sell.
    pub side: Side,
    /// How much to trade, in lots.
    pub qty: Qty,
    /// Limit or market.
    pub kind: OrderType,
}

/// Something the bot asks the exchange to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Place a new order.
    New(NewOrder),
    /// Cancel a resting order by id.
    Cancel(OrderId),
}

impl Command {
    /// The id of the order this command is about.
    pub fn order_id(&self) -> OrderId {
        match self {
            Command::New(order) => order.id,
            Command::Cancel(id) => *id,
        }
    }
}

/// Whether a fill added liquidity to the book or removed it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Liquidity {
    /// The order was resting and someone traded against it.
    Maker,
    /// The order arrived and traded against resting orders.
    Taker,
}

/// Why the exchange refused a command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RejectReason {
    /// An order with this id already exists.
    DuplicateId,
    /// The quantity was zero or negative.
    InvalidQty,
    /// The price was zero or negative.
    InvalidPrice,
    /// A cancel named an order that is not resting (never existed, or already
    /// filled or cancelled).
    UnknownOrder,
}

/// A report from the exchange about one of your orders.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecEvent {
    /// The exchange accepted the order.
    Ack {
        /// The accepted order.
        id: OrderId,
    },
    /// The exchange refused the command.
    Reject {
        /// The order the command was about.
        id: OrderId,
        /// Why it was refused.
        reason: RejectReason,
    },
    /// The order traded (fully or partly).
    Fill {
        /// The order that traded.
        id: OrderId,
        /// Side of the order that traded.
        side: Side,
        /// Price of this trade.
        price: Price,
        /// Quantity of this trade.
        qty: Qty,
        /// Maker or taker.
        liquidity: Liquidity,
        /// Fee for this fill; negative means a rebate.
        fee: Cash,
        /// Quantity still unfilled after this trade.
        leaves: Qty,
    },
    /// The order was removed from the book without (further) filling.
    Cancelled {
        /// The cancelled order.
        id: OrderId,
        /// Quantity that was still unfilled when it was cancelled.
        leaves: Qty,
    },
}

impl ExecEvent {
    /// The id of the order this event is about.
    pub fn id(&self) -> OrderId {
        match self {
            ExecEvent::Ack { id }
            | ExecEvent::Reject { id, .. }
            | ExecEvent::Fill { id, .. }
            | ExecEvent::Cancelled { id, .. } => *id,
        }
    }
}

/// One price level of the book: total quantity resting at one price.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Level {
    /// The price of this level.
    pub price: Price,
    /// Total quantity resting there.
    pub qty: Qty,
}

/// A picture of the book at one moment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BookSnapshot {
    /// Buy levels, best (highest price) first.
    pub bids: Vec<Level>,
    /// Sell levels, best (lowest price) first.
    pub asks: Vec<Level>,
}

/// Public market information that anyone can see.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MarketData {
    /// The current state of the book.
    Snapshot(BookSnapshot),
    /// A trade that just happened.
    Trade {
        /// Trade price.
        price: Price,
        /// Trade quantity.
        qty: Qty,
        /// The side of the order that started the trade.
        aggressor: Side,
    },
}

/// Everything the bot can be told: public data or a report on its own orders.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BotEvent {
    /// Public market data.
    Market(MarketData),
    /// A report about one of the bot's orders.
    Exec(ExecEvent),
}
