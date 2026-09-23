package org.slatelauncher.client.modules.qol

import java.util.Locale

private const val MAXIMUM_DIMENSION_NAME_LENGTH: Int = 64
private const val DEFAULT_COORDINATE_PRECISION: Int = 1
private const val MINIMUM_COORDINATE_PRECISION: Int = 0
private const val MAXIMUM_COORDINATE_PRECISION: Int = 3

public data class WorldPosition(
    public val x: Double,
    public val y: Double,
    public val z: Double,
    public val dimension: String,
) {
    init {
        require(listOf(x, y, z).all(Double::isFinite)) { "world coordinates must be finite" }
        require(dimension.isNotBlank() && dimension.length <= MAXIMUM_DIMENSION_NAME_LENGTH) {
            "dimension is invalid"
        }
    }
}

public object CoordinatesFormatter {
    public fun format(
        position: WorldPosition,
        decimals: Int = DEFAULT_COORDINATE_PRECISION,
    ): String {
        require(decimals in MINIMUM_COORDINATE_PRECISION..MAXIMUM_COORDINATE_PRECISION) {
            "coordinate precision must be between $MINIMUM_COORDINATE_PRECISION and $MAXIMUM_COORDINATE_PRECISION"
        }
        return String.format(
            Locale.ROOT,
            "% .${decimals}f  % .${decimals}f  % .${decimals}f  %s",
            position.x,
            position.y,
            position.z,
            position.dimension,
        ).trim()
    }
}
