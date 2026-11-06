/* tslint:disable */
/* eslint-disable */
/**
 * A compiled mapping, shaped as the editor's `AxisMappingBasis`.
 */
export interface ReferenceAxisMappingBasis {
    mappingId: string;
    inputAxisIds: string[];
    outputAxisIds: string[];
    basis: ReferenceVariationBasis;
}

/**
 * A design location, one entry per axis it sets.
 */
export type ReferenceAxisLocation = ReferenceAxisValue[];

/**
 * A font's compiled axis mappings, in order.
 */
export type AxisMappingBases = AxisMappingBasisData[];

/**
 * A location as one value per axis id.
 */
export type ReferenceAxisValues = Readonly<Record<string, number>>;

/**
 * A variation basis Rust compiled.
 */
export interface VariationBasisData {
    deltas: VariationBasisDelta[];
}

/**
 * An anchor's role in mark positioning, as export reads its name.
 */
export type AnchorKind = { kind: "base"; group: string } | { kind: "mark"; group: string } | { kind: "ligature"; group: string; index: number } | { kind: "componentMarker"; index: number } | { kind: "caret"; index: number } | { kind: "vcaret"; index: number } | { kind: "entry" } | { kind: "exit" } | { kind: "invalid" };

/**
 * An axis as variation reads it; the editor's `Axis` satisfies this.
 */
export interface VariationAxis {
    id: string;
    tag: string;
    name: string;
    role: VariationAxisRole;
    axisType: VariationAxisType;
    minimum?: number;
    default: number;
    maximum?: number;
    values?: number[];
}

/**
 * An axis mapping Rust compiled.
 */
export interface AxisMappingBasisData {
    mappingId: string;
    inputAxisIds: string[];
    outputAxisIds: string[];
    basis: VariationBasisData;
}

/**
 * Authored axis mappings; the editor's `AxisMapping[]` satisfies this.
 */
export type ReferenceAxisMappings = ReferenceAxisMapping[];

/**
 * Axes in order.
 */
export type VariationAxes = VariationAxis[];

/**
 * Compiled mappings, in the order given.
 */
export type ReferenceAxisMappingBases = ReferenceAxisMappingBasis[];

/**
 * Design-space bounds that normalize one axis for interpolation.
 */
export interface VariationDesignNormalization {
    axisId: string;
    minimum: number;
    default: number;
    maximum: number;
}

/**
 * One axis's support within a region, normalized.
 */
export interface VariationSupport {
    axisId: string;
    lower: number;
    peak: number;
    upper: number;
}

/**
 * One contribution: a region and the values it adds there.
 */
export interface VariationBasisDelta {
    region: VariationSupport[];
    values: Float64Array | number[];
}

/**
 * Source weights Rust compiled for an ordered source set.
 */
export interface InterpolationBasisData {
    sourceIds: string[];
    basis: VariationBasisData;
    designNormalization: VariationDesignNormalization[];
}

export interface ReferenceAxisMapping {
    id: string;
    name: string;
    inputs: string[];
    outputs: string[];
    points: ReferenceAxisMappingPoint[];
}

export interface ReferenceAxisMappingPoint {
    input: ReferenceLocation;
    output: ReferenceLocation;
}

export interface ReferenceAxisValue {
    axisId: string;
    value: number;
}

export interface ReferenceLocation {
    values: Readonly<Record<string, number>>;
}

export interface ReferenceSupport {
    axisId: string;
    lower: number;
    peak: number;
    upper: number;
}

export interface ReferenceVariationBasis {
    deltas: ReferenceVariationDelta[];
}

export interface ReferenceVariationDelta {
    region: ReferenceSupport[];
    values: number[];
}

export type VariationAxisRole = "external" | "internal";

export type VariationAxisType = "continuous" | "discrete";


/**
 * Returns the kind fontc reads from an anchor name.
 *
 * # Errors
 *
 * Returns a [`JsError`] when the kind cannot be converted to a JavaScript value.
 */
export function anchorKind(name: string): AnchorKind;

/**
 * Blends every component occurrence of a decomposed glyph as each master
 * places it, as compilers interpolate the glyph.
 *
 * Input: [`DecomposedGlyph`]. Output: each occurrence's blended points in
 * placed order, contour after contour, or null when any occurrence's masters
 * don't line up point for point.
 */
export function blendDecomposedGlyph(input: Float64Array): Float64Array;

/**
 * The weighted sum of source values: `interpolation::blend_values`.
 *
 * `values` holds each source's values one after another, `lengths[i]`
 * of them for source `i`; a source the editor lacks has length 0. None when
 * there is no source, one has no values, or two differ in length.
 */
export function blendSourceValues(weights: Float64Array, values: Float64Array, lengths: Uint32Array): Float64Array | undefined;

/**
 * `AxisMappingBasis::try_from` for each mapping, as the bridge sends them.
 */
export function compileAxisMappings(axes: VariationAxes, mappings: ReferenceAxisMappings): ReferenceAxisMappingBases;

/**
 * A component placement's matrix: `DecomposedTransform::to_matrix`.
 *
 * Input: [`DecomposedTransform`]. Output: [`Transform`], as the editor's
 * `MatModel` names it.
 */
export function composeTransform(input: Float64Array): Float64Array;

/**
 * Segments one contour and bounds each segment and the whole contour.
 *
 * Input: [`ContourPoints`]. Output: [`ContourGeometry`].
 */
export function contourGeometry(input: Float64Array): Float64Array;

/**
 * The placement a matrix makes: `DecomposedTransform::from_matrix`.
 *
 * Input: [`Transform`]. Output: [`DecomposedTransform`].
 */
export function decomposeMatrix(input: Float64Array): Float64Array;

/**
 * Positions for one frame of a prepared drag: the selected points moved by
 * the delta and the points its rules move, rules last. Empty for a released
 * or unknown handle.
 *
 * Input: [`DragFrame`]. Output: a list of [`DragMove`].
 */
export function dragMoves(input: Float64Array): Float64Array;

/**
 * A variation basis's values at a location, normalized by each axis's range.
 */
export function evaluateVariationBasis(basis: number, axes: number, location: Float64Array): Float64Array;

/**
 * Places a composite's components after an edit, anchored ones on their anchors.
 *
 * Input: a list of [`FollowingComponent`]. Output: a list of [`Following`],
 * one per component.
 */
export function followAnchors(input: Float64Array): Float64Array;

/**
 * Returns the id of a name, interning it the first time.
 */
export function internName(name: string): number;

/**
 * One weight per source of an interpolation basis at a design location.
 *
 * A region naming an axis the basis or `axes` lack contributes nothing.
 */
export function interpolationWeights(basis: number, axes: number, location: Float64Array): Float64Array;

/**
 * Maps an external location to its design location, in `axes` order.
 */
export function mapAxisLocation(mappings: number, axes: number, external: Float64Array): Float64Array;

/**
 * `map_location`: an external location through the authored mappings to a
 * design location, one value per axis it sets.
 */
export function mapLocation(axes: VariationAxes, mappings: ReferenceAxisMappings, location: ReferenceAxisValues): ReferenceAxisLocation;

/**
 * Returns the name an id stands for; empty for an unknown id.
 */
export function nameOf(id: number): string;

/**
 * Matches the drag rules for dragging the selected points, keeping the
 * points as the drag found them until [`release_drag`].
 *
 * Input: [`DragSelection`]. Output: [`PreparedDragHandle`].
 */
export function prepareDrag(input: Float64Array): Float64Array;

/**
 * Propagates one glyph's anchors at one location, as export compiles them.
 *
 * Input: a [`PropagationGlyph`]. Output: [`PropagatedAnchors`].
 */
export function propagateAnchors(input: Float64Array): Float64Array;

/**
 * Registers axes in order, for locations passed as numbers in that order.
 *
 * # Errors
 *
 * Returns a [`JsError`] when `axes` does not match the expected shape.
 */
export function registerAxes(axes: VariationAxes): number;

/**
 * Registers a font's axis mappings, in order.
 *
 * # Errors
 *
 * Returns a [`JsError`] when `bases` does not match the expected shape.
 */
export function registerAxisMappings(bases: AxisMappingBases): number;

/**
 * Registers an interpolation basis.
 *
 * # Errors
 *
 * Returns a [`JsError`] when `basis` does not match the expected shape.
 */
export function registerInterpolationBasis(basis: InterpolationBasisData): number;

/**
 * Registers a variation basis evaluated with each axis's own range.
 *
 * # Errors
 *
 * Returns a [`JsError`] when `basis` does not match the expected shape.
 */
export function registerVariationBasis(basis: VariationBasisData): number;

export function releaseAxes(handle: number): void;

export function releaseAxisMappings(handle: number): void;

/**
 * Forgets a prepared drag. Releasing an unknown handle does nothing.
 */
export function releaseDrag(handle: number): void;

export function releaseInterpolationBasis(handle: number): void;

export function releaseVariationBasis(handle: number): void;

/**
 * The external location a design location's mapping came from, in `axes`
 * order, NaN on internal axes: `unmap_location_with_bases`.
 */
export function unmapAxisLocation(mappings: number, axes: number, design: Float64Array): Float64Array;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly __externref_table_alloc: () => number;
    readonly __externref_table_dealloc: (a: number) => void;
    readonly __wbindgen_exn_store: (a: number) => void;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_start: () => void;
    readonly anchorKind: (a: number, b: number) => [number, number, number];
    readonly blendDecomposedGlyph: (a: number, b: number) => [number, number];
    readonly blendSourceValues: (a: number, b: number, c: number, d: number, e: number, f: number) => [number, number];
    readonly compileAxisMappings: (a: any, b: any) => [number, number, number];
    readonly composeTransform: (a: number, b: number) => [number, number];
    readonly contourGeometry: (a: number, b: number) => [number, number];
    readonly decomposeMatrix: (a: number, b: number) => [number, number];
    readonly dragMoves: (a: number, b: number) => [number, number];
    readonly evaluateVariationBasis: (a: number, b: number, c: number, d: number) => [number, number];
    readonly followAnchors: (a: number, b: number) => [number, number];
    readonly internName: (a: number, b: number) => number;
    readonly interpolationWeights: (a: number, b: number, c: number, d: number) => [number, number];
    readonly mapAxisLocation: (a: number, b: number, c: number, d: number) => [number, number];
    readonly mapLocation: (a: any, b: any, c: any) => [number, number, number];
    readonly memory: WebAssembly.Memory;
    readonly nameOf: (a: number) => [number, number];
    readonly prepareDrag: (a: number, b: number) => [number, number];
    readonly propagateAnchors: (a: number, b: number) => [number, number];
    readonly registerAxes: (a: any) => [number, number, number];
    readonly registerAxisMappings: (a: any) => [number, number, number];
    readonly registerInterpolationBasis: (a: any) => [number, number, number];
    readonly registerVariationBasis: (a: any) => [number, number, number];
    readonly releaseAxes: (a: number) => void;
    readonly releaseAxisMappings: (a: number) => void;
    readonly releaseDrag: (a: number) => void;
    readonly releaseInterpolationBasis: (a: number) => void;
    readonly releaseVariationBasis: (a: number) => void;
    readonly unmapAxisLocation: (a: number, b: number, c: number, d: number) => [number, number];
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
