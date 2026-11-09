<?php

declare(strict_types=1);

/*
 * Writes survey_basic_phpwriter.sav (same content as survey_basic.sav) with the independent
 * pure-PHP writer openstatspec/sav (https://github.com/OpenStatSpec/php-sav), which can
 * also store the multiple-response sets and the weight variable that pyreadstat cannot.
 *
 * Built with php-sav 3.1.1 (commit c4ff108) on PHP 8.5.9. Run after generate.py:
 *
 *     php tests/fixtures/generate_php.php /path/to/php-sav
 *
 * The argument is a php-sav checkout (or any directory) that has vendor/autoload.php for the
 * library. Input is survey_basic_phpwriter.input.json, written by generate.py.
 */

use Vendor\Sav\Alignment;
use Vendor\Sav\Dataset;
use Vendor\Sav\FileMetadata;
use Vendor\Sav\FileTechnicalMetadata;
use Vendor\Sav\Measure;
use Vendor\Sav\MissingValues;
use Vendor\Sav\MultipleResponseSet;
use Vendor\Sav\MultipleResponseSetType;
use Vendor\Sav\ValueLabel;
use Vendor\Sav\ValueLabelSet;
use Vendor\Sav\Variable;
use Vendor\Sav\VariableDictionary;
use Vendor\Sav\VariableFormat;
use Vendor\Sav\VariableMetadata;
use Vendor\Sav\VariableType;
use Vendor\Sav\Writer;

$autoload = ($argv[1] ?? '') . '/vendor/autoload.php';
is_file($autoload) || exit("usage: php generate_php.php /path/to/php-sav\n");
require $autoload;

ini_set('serialize_precision', '-1');
$base = __DIR__ . '/survey_basic_phpwriter';
$json = (string) file_get_contents($base . '.input.json');
$input = json_decode($json, true, flags: JSON_THROW_ON_ERROR);
$expected = $input['expected'];

$formatCodes = ['A' => Variable::FORMAT_TYPE_A, 'F' => Variable::FORMAT_TYPE_F, 'DATE' => Variable::FORMAT_TYPE_DATE];
$variables = [];
$position = 1; // 1-based position among the variable records, where a string takes one per 8 bytes
foreach ($expected['variables'] as $v) {
    preg_match('/^([A-Z]+)(\d+)(?:\.(\d+))?$/', $v['format'], $m) || throw new RuntimeException('Bad format ' . $v['format']);
    $format = new VariableFormat($formatCodes[$m[1]], (int) $m[2], (int) ($m[3] ?? 0));
    $isString = 'string' === $v['type'];
    $labels = [];
    foreach ($v['value_labels'] as $value => $label) {
        $labels[] = new ValueLabel($isString ? (string) $value : $value + 0, $label);
    }
    $variables[] = new VariableMetadata(
        name: $v['name'],
        type: $isString ? VariableType::STRING : VariableType::NUMERIC,
        width: $v['width'] ?? 0,
        printFormat: $format,
        writeFormat: $format,
        label: $v['label'],
        valueLabels: [] === $labels ? null : new ValueLabelSet($labels, [$v['name']]),
        missingValues: match (true) {
            null !== $v['missing_range'] && [] !== $v['missing_values'] => MissingValues::rangeAndValue($v['missing_range'][0], $v['missing_range'][1], $v['missing_values'][0]),
            null !== $v['missing_range'] => MissingValues::range($v['missing_range'][0], $v['missing_range'][1]),
            [] !== $v['missing_values'] => MissingValues::discrete(...$v['missing_values']),
            default => null,
        },
        measure: Measure::{strtoupper($v['measure'])},
        alignment: $isString ? Alignment::LEFT : Alignment::RIGHT,
        // php-sav 3.1.1 otherwise writes the weight index as the plain variable number, which is
        // wrong once a string variable precedes the weight variable.
        // shortcut: valid for widths up to 255 only, extend before adding a very long string here.
        dictionaryIndex: $position,
    );
    $position += $isString ? intdiv($v['width'] + 7, 8) : 1;
}

$mrSets = [];
foreach ($expected['multiple_response_sets'] as $set) {
    $mrSets[] = new MultipleResponseSet(
        name: $set['name'],
        type: MultipleResponseSetType::from($set['type']),
        variableNames: $set['variables'],
        label: $set['label'],
        countedValue: $set['counted_value'],
    );
}

$writer = new Writer(new Dataset(
    dictionary: new VariableDictionary($variables),
    rows: $input['cases'],
    metadata: new FileMetadata(
        label: $expected['file_label'] ?? null,
        weightVariableName: $expected['weight_variable'],
        multipleResponseSets: $mrSets,
    ),
    technicalMetadata: new FileTechnicalMetadata(
        encoding: $expected['encoding'],
        compression: ['none' => 0, 'bytecode' => 1, 'zlib' => 2][$expected['compression']],
    ),
));
$writer->save($base . '.sav');
$writer->close();

file_put_contents(
    $base . '.expected.json',
    // Decoded as objects here so that an empty value_labels stays {} instead of becoming [].
    json_encode(json_decode($json, flags: JSON_THROW_ON_ERROR)->expected, JSON_PRETTY_PRINT | JSON_UNESCAPED_UNICODE | JSON_UNESCAPED_SLASHES | JSON_PRESERVE_ZERO_FRACTION | JSON_THROW_ON_ERROR) . "\n",
);
echo "wrote survey_basic_phpwriter.sav and survey_basic_phpwriter.expected.json\n";
