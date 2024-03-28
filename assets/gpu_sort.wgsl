struct Entry {
    originalIndex: u32,
    hash: u32,
    key: u32,
};

@group(0) @binding(0) var<storage, read_write> entries: array<Entry>;
@group(0) @binding(1) var<uniform> constants: Constants;
@group(0) @binding(2) var<storage, read_write> offsets: array<u32>;

struct Constants {
    numEntries: u32,
    groupWidth: u32,
    groupHeight: u32,
    stepIndex: u32,
};

// Sort the given entries by their keys (smallest to largest)
// This is done using bitonic merge sort, and takes multiple iterations
@compute @workgroup_size(128, 1, 1)
fn sort(@builtin(global_invocation_id) invocation_id: vec3<u32>) {
    let i: u32 = invocation_id.x;

    let hIndex: u32 = i & (constants.groupWidth - 1);
    let indexLeft: u32 = hIndex + (constants.groupHeight + 1) * (i / constants.groupWidth);
    let rightStepSize: u32 = if constants.stepIndex == 0 { constants.groupHeight - 2 * hIndex } else { (constants.groupHeight + 1) / 2 };
    let indexRight: u32 = indexLeft + rightStepSize;

    // Exit if out of bounds (for non-power of 2 input sizes)
    if indexRight >= constants.numEntries { return; }

    let valueLeft: u32 = entries[indexLeft].key;
    let valueRight: u32 = entries[indexRight].key;

    // Swap entries if value is descending
    if valueLeft > valueRight {
        let temp: Entry = entries[indexLeft];
        entries[indexLeft] = entries[indexRight];
        entries[indexRight] = temp;
    }
}


// Calculate offsets into the sorted Entries buffer (used for spatial hashing).
// For example, given an Entries buffer sorted by key like so: {2, 2, 2, 3, 6, 6, 9, 9, 9, 9}
// The resulting Offsets calculated here should be:            {-, -, 0, 3, -, -, 4, -, -, 6}
// (where '-' represents elements that won't be read/written)
// 
// Usage example:
// Say we have a particular particle P, and we want to know which particles are in the same grid cell as it.
// First we would calculate the Key of P based on its position. Let's say in this example that Key = 9.
// Next we can look up Offsets[Key] to get: Offsets[9] = 6
// This tells us that SortedEntries[6] is the first particle that's in the same cell as P.
// We can then loop until we reach a particle with a different cell key in order to iterate over all the particles in the cell.
// 
// NOTE: offsets buffer must filled with values equal to (or greater than) its length to ensure that this works correctly
@compute @workgroup_size(128, 1, 1)
fn calculateOffsets(@builtin(global_invocation_id) invocation_id: vec3<u32>) {
    let i: u32 = invocation_id.x;
    let null: u32 = constants.numEntries;

    if i >= constants.numEntries { return; }

    let key: u32 = entries[i].key;
    let keyPrev: u32 = if i == 0 { null } else { entries[i - 1].key };

    if key != keyPrev {
        offsets[key] = i;
    }
}