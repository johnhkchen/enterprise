package dev.b28.enterprise.contract;

/**
 * Who carries a delivery, who bears the road, and who pays the tolls on the way: the shipping terms of a contract,
 * after the trade's own Incoterms. Tolls stand for duties. Risk is the road's: an installment under terms where the
 * buyer bears the road counts as delivered once the seller's caravan leaves with it; where the seller bears it, only
 * once it's there. (When raids come, goods lost on the road are lost to whoever bears it.)
 */
public enum Incoterm {
    /** Ex Works: the buyer collects at the seller's, with its own caravan, and bears everything from there. */
    EXW("Ex Works", false, false, false, "the buyer collects at the seller's and bears the road"),
    /** Carriage Paid To: the seller's caravan carries it, but it's the buyer's once on the road; the buyer pays tolls. */
    CPT("Carriage Paid To", true, false, false,
            "the seller carries it; it's the buyer's once on the road, and the buyer pays the tolls"),
    /** Delivered At Place: the seller carries it and bears the road until it's there; the buyer pays the tolls. */
    DAP("Delivered At Place", true, true, false,
            "the seller carries it and bears the road until it's there; the buyer pays the tolls"),
    /** Delivered Duty Paid: the seller carries it, bears the road and pays the tolls: the buyer just receives it. */
    DDP("Delivered Duty Paid", true, true, true, "the seller carries it, bears the road and pays the tolls");

    public final String title;
    /** The seller's caravans carry it (otherwise the buyer's collect it). */
    public final boolean sellerCarries;
    /** The seller bears the road: it's delivered only once it's there. */
    public final boolean sellerBearsRoad;
    /** The seller pays the tolls on the way (otherwise the buyer pays them on top of the price). */
    public final boolean sellerPaysTolls;
    public final String does;

    Incoterm(String title, boolean sellerCarries, boolean sellerBearsRoad, boolean sellerPaysTolls, String does) {
        this.title = title;
        this.sellerCarries = sellerCarries;
        this.sellerBearsRoad = sellerBearsRoad;
        this.sellerPaysTolls = sellerPaysTolls;
        this.does = does;
    }

    /**
     * The price per item under these terms, from the price at the seller's ({@code exWorks}): plus the carriage
     * when the seller carries, and the tolls when it pays them.
     */
    public double price(double exWorks, double carriageEach, double tollsEach) {
        return exWorks + (sellerCarries ? carriageEach : 0) + (sellerPaysTolls ? tollsEach : 0);
    }
}
