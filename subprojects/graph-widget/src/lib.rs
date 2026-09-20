/* src/lib.rs
 *
 * Copyright 2026 Mission Center Developers
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <http://www.gnu.org/licenses/>.
 *
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

pub mod render;
pub mod widget;

// Re-exported so consumers of this crate need not depend on core directly.
pub use mc_graph_core::{
    animation, dataset, scaling, AnimationFrame, Dataset, DatasetGroup, DatasetPoints,
    DatasetSettings, FillingSettings, RoundingSettings, ScalingSettings, MAX_DATA_POINTS,
    MIN_DATA_POINTS,
};
pub use widget::GraphWidget;
